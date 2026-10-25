import { existsSync, lstatSync, readFileSync, readdirSync, realpathSync, statSync } from 'node:fs';
import { basename, dirname, extname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const EXPECTED_PRODUCT_NAME = 'Nexora';
const EXPECTED_IDENTIFIER = 'com.nexora.desktop';
const REQUIRED_ICONS = ['32x32.png', '128x128.png', '128x128@2x.png', 'icon.ico'];
const EXPECTED_PNG_DIMENSIONS = new Map([
  ['32x32.png', { width: 32, height: 32 }],
  ['128x128.png', { width: 128, height: 128 }],
  ['128x128@2x.png', { width: 256, height: 256 }],
]);
const EXPECTED_MAIN_PERMISSIONS = [
  'core:default',
  'core:window:allow-minimize',
  'core:window:allow-toggle-maximize',
  'core:window:allow-close',
  'core:window:allow-start-dragging',
];
const FORBIDDEN_PACKAGE_EXTENSIONS = new Set([
  '.gguf',
  '.ggml',
  '.safetensors',
  '.onnx',
  '.pt',
  '.pth',
  '.weights',
  '.model',
  '.bin',
  '.part',
  '.exe',
  '.dll',
  '.pfx',
  '.p12',
  '.pem',
  '.key',
]);

function fail(message) {
  throw new Error(`Installer contract failed: ${message}`);
}

function isPathWithin(parentPath, candidatePath) {
  const candidateRelative = relative(parentPath, candidatePath);
  return (
    candidateRelative === '' ||
    (!isAbsolute(candidateRelative) &&
      candidateRelative !== '..' &&
      !candidateRelative.startsWith(`..${sep}`))
  );
}

function readJson(filePath) {
  try {
    return JSON.parse(readFileSync(filePath, 'utf8'));
  } catch (error) {
    fail(`could not parse ${filePath}: ${error.message}`);
  }
}

function readCargoPackageVersion(filePath) {
  let contents;
  try {
    contents = readFileSync(filePath, 'utf8');
  } catch (error) {
    fail(`could not read ${filePath}: ${error.message}`);
  }

  const packageHeader = /^\[package\][ \t]*$/m.exec(contents);
  if (!packageHeader) fail(`could not find the [package] section in ${filePath}`);
  const packageStart = packageHeader.index + packageHeader[0].length;
  const remaining = contents.slice(packageStart);
  const nextSection = /^\[[^\]\r\n]+\][ \t]*$/m.exec(remaining);
  const packageSection = nextSection ? remaining.slice(0, nextSection.index) : remaining;
  const version = packageSection.match(/^version\s*=\s*"([^"]+)"\s*(?:#.*)?$/m)?.[1];
  if (!version) fail(`could not determine the [package] version in ${filePath}`);
  return version;
}

function listFilesRecursively(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const filePath = join(directory, entry.name);
    if (entry.isSymbolicLink()) fail(`production frontend contains a symbolic link: ${filePath}`);
    if (entry.isDirectory()) files.push(...listFilesRecursively(filePath));
    else if (entry.isFile()) files.push(filePath);
  }
  return files;
}

function checkPng(iconPath) {
  const contents = readFileSync(iconPath);
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  if (
    contents.length < 24 ||
    !contents.subarray(0, 8).equals(signature) ||
    contents.readUInt32BE(8) !== 13 ||
    contents.toString('ascii', 12, 16) !== 'IHDR'
  ) {
    fail(`referenced icon is not a complete PNG IHDR: ${iconPath}`);
  }
  const width = contents.readUInt32BE(16);
  const height = contents.readUInt32BE(20);
  if (width === 0 || height === 0) fail(`referenced PNG icon has empty dimensions: ${iconPath}`);
  return { width, height };
}

function checkIco(iconPath) {
  const contents = readFileSync(iconPath);
  if (contents.length < 22 || contents.readUInt16LE(0) !== 0 || contents.readUInt16LE(2) !== 1) {
    fail(`referenced icon is not a valid ICO directory: ${iconPath}`);
  }
  const entries = contents.readUInt16LE(4);
  const directoryEnd = 6 + entries * 16;
  if (entries === 0 || contents.length < directoryEnd) {
    fail(`referenced ICO contains no complete image entries: ${iconPath}`);
  }

  for (let index = 0; index < entries; index += 1) {
    const entryOffset = 6 + index * 16;
    const entryWidth = contents[entryOffset] || 256;
    const entryHeight = contents[entryOffset + 1] || 256;
    const imageBytes = contents.readUInt32LE(entryOffset + 8);
    const imageOffset = contents.readUInt32LE(entryOffset + 12);
    if (
      imageBytes < 24 ||
      imageOffset < directoryEnd ||
      imageOffset + imageBytes > contents.length
    ) {
      fail(`referenced ICO has an out-of-range or incomplete image entry: ${iconPath}`);
    }

    const image = contents.subarray(imageOffset, imageOffset + imageBytes);
    const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    if (image.subarray(0, 8).equals(pngSignature)) {
      if (
        image.readUInt32BE(8) !== 13 ||
        image.toString('ascii', 12, 16) !== 'IHDR' ||
        image.readUInt32BE(16) !== entryWidth ||
        image.readUInt32BE(20) !== entryHeight
      ) {
        fail(`referenced ICO has a malformed or dimension-mismatched PNG entry: ${iconPath}`);
      }
    } else if (image.readUInt32LE(0) < 40 || image.readUInt32LE(0) > imageBytes) {
      fail(`referenced ICO has an unsupported or malformed image entry: ${iconPath}`);
    }
  }

  return entries;
}

/**
 * Validate the source-level installer contract and built frontend payload.
 * This is intentionally a configuration/asset check, not a Windows build or
 * an inspection of the generated NSIS executable.
 */
export function validateInstallerContract(root = repositoryRoot) {
  root = resolve(root);
  const packageJson = readJson(join(root, 'package.json'));
  const desktopPackageJson = readJson(join(root, 'apps', 'desktop', 'package.json'));
  const tauriConfigPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const tauriConfig = readJson(tauriConfigPath);
  const cargoVersion = readCargoPackageVersion(join(root, 'apps', 'desktop', 'src-tauri', 'Cargo.toml'));
  const versions = [
    ['root package.json', packageJson.version],
    ['desktop package.json', desktopPackageJson.version],
    ['Tauri configuration', tauriConfig.version],
    ['Tauri Cargo package', cargoVersion],
  ];
  const version = versions[0][1];
  if (!version || versions.some(([, value]) => value !== version)) {
    fail(`product versions must be present and identical (${versions.map(([name, value]) => `${name}=${value ?? '<missing>'}`).join(', ')})`);
  }

  if (tauriConfig.productName !== EXPECTED_PRODUCT_NAME) {
    fail(`Tauri productName must remain ${EXPECTED_PRODUCT_NAME}`);
  }
  if (tauriConfig.identifier !== EXPECTED_IDENTIFIER) {
    fail(`Tauri identifier must remain ${EXPECTED_IDENTIFIER}`);
  }
  const configuredWindows = tauriConfig.app?.windows;
  if (
    !Array.isArray(configuredWindows) ||
    configuredWindows.length !== 1 ||
    configuredWindows[0]?.label !== 'main' ||
    configuredWindows[0]?.title !== EXPECTED_PRODUCT_NAME ||
    configuredWindows[0]?.visible !== true
  ) {
    fail('the production desktop must retain one visible main Nexora window');
  }
  const capabilitiesRoot = join(dirname(tauriConfigPath), 'capabilities');
  if (!existsSync(capabilitiesRoot)) fail('the reviewed main-window capability file is missing');
  const capabilityFiles = readdirSync(capabilitiesRoot, { withFileTypes: true });
  if (
    capabilityFiles.length !== 1 ||
    capabilityFiles[0].name !== 'main.json' ||
    !capabilityFiles[0].isFile()
  ) {
    fail('the production desktop must use only the reviewed main-window capability file');
  }
  const mainCapability = readJson(join(capabilitiesRoot, 'main.json'));
  const configuredPermissions = Array.isArray(mainCapability.permissions)
    ? [...mainCapability.permissions].sort()
    : [];
  const expectedPermissions = [...EXPECTED_MAIN_PERMISSIONS].sort();
  if (
    mainCapability.identifier !== 'main' ||
    JSON.stringify(mainCapability.windows) !== JSON.stringify(['main']) ||
    JSON.stringify(configuredPermissions) !== JSON.stringify(expectedPermissions)
  ) {
    fail('the main-window capability must retain the reviewed least-privilege permissions');
  }
  if (tauriConfig.build?.beforeBuildCommand !== 'pnpm build') {
    fail('Tauri beforeBuildCommand must build the pinned production frontend with pnpm build');
  }

  const frontendDistSetting = tauriConfig.build?.frontendDist;
  if (frontendDistSetting !== '../ui/dist') {
    fail('Tauri frontendDist must point to the desktop production UI at ../ui/dist');
  }
  const tauriRoot = dirname(tauriConfigPath);
  const frontendDist = resolve(tauriRoot, frontendDistSetting);
  const relativeFrontendDist = relative(root, frontendDist);
  if (!isPathWithin(root, frontendDist) || !existsSync(join(frontendDist, 'index.html'))) {
    fail(`production frontend entry is missing or outside the repository: ${relativeFrontendDist}`);
  }
  const realRoot = realpathSync(root);
  let frontendPath = realRoot;
  for (const component of relativeFrontendDist.split(sep).filter(Boolean)) {
    frontendPath = join(frontendPath, component);
    if (lstatSync(frontendPath).isSymbolicLink()) {
      fail(`production frontend path contains a symbolic link: ${relative(root, frontendPath)}`);
    }
  }
  const realFrontendDist = realpathSync(frontendPath);
  if (!isPathWithin(realRoot, realFrontendDist) || !statSync(realFrontendDist).isDirectory()) {
    fail(`production frontend resolves outside the repository or is not a directory: ${relativeFrontendDist}`);
  }

  const bundle = tauriConfig.bundle;
  if (bundle?.active !== true || JSON.stringify(bundle.targets) !== JSON.stringify(['nsis'])) {
    fail('the active Windows installer target must remain NSIS only');
  }
  if (bundle.publisher !== EXPECTED_PRODUCT_NAME) {
    fail(`installer publisher must remain ${EXPECTED_PRODUCT_NAME}`);
  }
  if (!bundle.category || !bundle.shortDescription || !bundle.longDescription) {
    fail('installer product metadata must include category and descriptions');
  }
  if (bundle.resources != null && !(Array.isArray(bundle.resources) && bundle.resources.length === 0)) {
    fail('installer resources must not embed unverified model or separately supplied runtime files');
  }
  if (bundle.externalBin != null && !(Array.isArray(bundle.externalBin) && bundle.externalBin.length === 0)) {
    fail('installer externalBin must not embed a runtime or unverified model executable');
  }

  const windows = bundle.windows;
  if (windows?.nsis?.installMode !== 'currentUser') {
    fail('NSIS must retain the per-user currentUser installation mode');
  }
  if (windows.nsis.template != null || windows.nsis.installerHooks != null) {
    fail('custom NSIS templates/hooks are not allowed to bypass the reviewed install, shortcut, or data-preservation contract');
  }
  if (windows?.webviewInstallMode?.type !== 'downloadBootstrapper' || windows.webviewInstallMode.silent !== true) {
    fail('WebView2 must use the configured silent Evergreen bootstrapper strategy');
  }

  const iconPaths = bundle.icon;
  if (!Array.isArray(iconPaths) || REQUIRED_ICONS.some((required) => !iconPaths.includes(`icons/${required}`))) {
    fail(`Tauri bundle must reference all required icons: ${REQUIRED_ICONS.join(', ')}`);
  }
  const iconRoot = join(tauriRoot, 'icons');
  const iconDimensions = {};
  for (const iconName of REQUIRED_ICONS) {
    const iconPath = join(iconRoot, iconName);
    if (!existsSync(iconPath) || statSync(iconPath).size === 0) fail(`required installer icon is missing or empty: ${iconPath}`);
    if (iconName.endsWith('.png')) {
      const dimensions = checkPng(iconPath);
      const expectedDimensions = EXPECTED_PNG_DIMENSIONS.get(iconName);
      if (dimensions.width !== expectedDimensions.width || dimensions.height !== expectedDimensions.height) {
        fail(
          `referenced PNG icon ${iconPath} must be ${expectedDimensions.width}x${expectedDimensions.height}; ` +
            `found ${dimensions.width}x${dimensions.height}`,
        );
      }
      iconDimensions[iconName] = dimensions;
    } else {
      checkIco(iconPath);
    }
  }

  const files = listFilesRecursively(realFrontendDist);
  for (const filePath of files) {
    const lowerName = filePath.toLowerCase();
    const extension = extname(lowerName);
    const baseName = basename(lowerName);
    if (FORBIDDEN_PACKAGE_EXTENSIONS.has(extension) || /^\.env(?:\.|$)/i.test(baseName)) {
      fail(`production frontend contains a model, runtime, credential, or installer payload: ${relative(root, filePath)}`);
    }
  }

  const bytes = files.reduce((sum, filePath) => sum + statSync(filePath).size, 0);
  return {
    productName: tauriConfig.productName,
    identifier: tauriConfig.identifier,
    version,
    target: bundle.targets[0],
    frontendDist: relative(root, frontendDist),
    frontendFileCount: files.length,
    frontendBytes: bytes,
    iconDimensions,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const report = validateInstallerContract();
    console.log(
      `Installer contract passed: ${report.productName} ${report.version} (${report.identifier}), ${report.target}, ` +
        `${report.frontendFileCount} frontend assets / ${report.frontendBytes} bytes; no model/runtime/credential payloads found.`,
    );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
