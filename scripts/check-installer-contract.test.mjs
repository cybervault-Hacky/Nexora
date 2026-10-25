import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { validateInstallerContract } from './check-installer-contract.mjs';

function makeFixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'nexora-installer-contract-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));

  const writeJson = (path, value) => {
    mkdirSync(join(root, path, '..'), { recursive: true });
    writeFileSync(join(root, path), `${JSON.stringify(value, null, 2)}\n`);
  };
  const tauriRoot = join(root, 'apps', 'desktop', 'src-tauri');
  const iconRoot = join(tauriRoot, 'icons');
  const distRoot = join(root, 'apps', 'desktop', 'ui', 'dist');
  mkdirSync(iconRoot, { recursive: true });
  mkdirSync(distRoot, { recursive: true });

  writeJson('package.json', { version: '0.1.0' });
  writeJson('apps/desktop/package.json', { version: '0.1.0' });
  writeFileSync(join(tauriRoot, 'Cargo.toml'), '[package]\nname = "nexora"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\n');
  writeJson('apps/desktop/src-tauri/capabilities/main.json', {
    identifier: 'main',
    windows: ['main'],
    permissions: [
      'core:default',
      'core:window:allow-minimize',
      'core:window:allow-toggle-maximize',
      'core:window:allow-close',
      'core:window:allow-start-dragging',
    ],
  });
  writeJson('apps/desktop/src-tauri/tauri.conf.json', {
    productName: 'Nexora',
    version: '0.1.0',
    identifier: 'com.nexora.desktop',
    build: { beforeBuildCommand: 'pnpm build', frontendDist: '../ui/dist' },
    app: { windows: [{ label: 'main', title: 'Nexora', visible: true }] },
    bundle: {
      active: true,
      targets: ['nsis'],
      publisher: 'Nexora',
      category: 'DeveloperTool',
      shortDescription: 'Local-first AI coding studio',
      longDescription: 'Nexora desktop application',
      icon: ['icons/32x32.png', 'icons/128x128.png', 'icons/128x128@2x.png', 'icons/icon.ico'],
      windows: {
        webviewInstallMode: { type: 'downloadBootstrapper', silent: true },
        nsis: { installMode: 'currentUser' },
      },
    },
  });
  writeFileSync(join(distRoot, 'index.html'), '<!doctype html><html><body>Nexora</body></html>\n');

  for (const [name, width, height] of [
    ['32x32.png', 32, 32],
    ['128x128.png', 128, 128],
    ['128x128@2x.png', 256, 256],
  ]) {
    const png = Buffer.alloc(24);
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(png, 0);
    png.writeUInt32BE(13, 8);
    png.write('IHDR', 12, 'ascii');
    png.writeUInt32BE(width, 16);
    png.writeUInt32BE(height, 20);
    writeFileSync(join(iconRoot, name), png);
  }
  const icoImage = Buffer.alloc(24);
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(icoImage, 0);
  icoImage.writeUInt32BE(13, 8);
  icoImage.write('IHDR', 12, 'ascii');
  icoImage.writeUInt32BE(32, 16);
  icoImage.writeUInt32BE(32, 20);
  const ico = Buffer.alloc(6 + 16 + icoImage.length);
  ico.writeUInt16LE(1, 2);
  ico.writeUInt16LE(1, 4);
  ico[6] = 32;
  ico[7] = 32;
  ico.writeUInt16LE(1, 10);
  ico.writeUInt16LE(32, 12);
  ico.writeUInt32LE(icoImage.length, 14);
  ico.writeUInt32LE(22, 18);
  icoImage.copy(ico, 22);
  writeFileSync(join(iconRoot, 'icon.ico'), ico);

  return { root, distRoot, iconRoot };
}

test('accepts consistent per-user NSIS metadata and a built frontend without runtime/model payloads', (t) => {
  const { root } = makeFixture(t);
  const result = validateInstallerContract(root);
  assert.equal(result.productName, 'Nexora');
  assert.equal(result.identifier, 'com.nexora.desktop');
  assert.equal(result.version, '0.1.0');
  assert.equal(result.target, 'nsis');
  assert.equal(result.frontendFileCount, 1);
  assert.equal(result.frontendBytes > 0, true);
});

test('rejects a hidden production main window', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.app.windows[0].visible = false;
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /one visible main Nexora window/);
});

test('rejects additional Tauri permissions outside the reviewed main-window capability', (t) => {
  const { root } = makeFixture(t);
  const capabilityPath = join(root, 'apps', 'desktop', 'src-tauri', 'capabilities', 'main.json');
  const capability = JSON.parse(readFileSync(capabilityPath, 'utf8'));
  capability.permissions.push('shell:allow-execute');
  writeFileSync(capabilityPath, `${JSON.stringify(capability, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /reviewed least-privilege permissions/);
});

test('rejects a drifted installer publisher identity', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle.publisher = 'Different Publisher';
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /publisher must remain Nexora/);
});

test('rejects custom NSIS hooks that bypass reviewed install and data behavior', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle.windows.nsis.installerHooks = 'installer-hooks.nsh';
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /custom NSIS templates\/hooks/);
});

test('rejects version drift between package and Tauri metadata', (t) => {
  const { root } = makeFixture(t);
  writeFileSync(join(root, 'apps', 'desktop', 'package.json'), '{"version":"0.2.0"}\n');
  assert.throws(() => validateInstallerContract(root), /versions must be present and identical/);
});

test('rejects a missing production frontend entry point', (t) => {
  const { root, distRoot } = makeFixture(t);
  rmSync(join(distRoot, 'index.html'));
  assert.throws(() => validateInstallerContract(root), /production frontend entry is missing/);
});

test('rejects frontendDist drift from the reviewed in-repository UI path', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.build.frontendDist = '../../../../outside';
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /Tauri frontendDist must point to the desktop production UI/);
});

test('rejects a production frontend directory redirected through a symlink', (t) => {
  const { root, distRoot } = makeFixture(t);
  const redirectedFrontend = join(root, 'redirected-frontend');
  mkdirSync(redirectedFrontend);
  writeFileSync(join(redirectedFrontend, 'index.html'), '<!doctype html>');
  rmSync(distRoot, { recursive: true });
  symlinkSync(redirectedFrontend, distRoot, process.platform === 'win32' ? 'junction' : 'dir');
  assert.throws(() => validateInstallerContract(root), /production frontend path contains a symbolic link/);
});

test('rejects model weights and credential files in the frontend payload', (t) => {
  const { root, distRoot } = makeFixture(t);
  writeFileSync(join(distRoot, 'qwen-model.gguf'), 'must not be packaged');
  assert.throws(() => validateInstallerContract(root), /model, runtime, credential, or installer payload/);
});

test('rejects a machine-wide installer mode', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle.windows.nsis.installMode = 'perMachine';
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /currentUser installation mode/);
});


test('rejects a non-silent or non-bootstrapper WebView2 install mode', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle.windows.webviewInstallMode.silent = false;
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /WebView2 must use/);
});

test('rejects accidental sidecar/model resource declarations', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle.externalBin = ['binaries/llama-server'];
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /externalBin must not embed/);
});

test('rejects a missing referenced Windows icon', (t) => {
  const { root, iconRoot } = makeFixture(t);
  rmSync(join(iconRoot, 'icon.ico'));
  assert.throws(() => validateInstallerContract(root), /required installer icon is missing/);
});

test('rejects a PNG icon whose dimensions do not match the configured Windows size', (t) => {
  const { root, iconRoot } = makeFixture(t);
  const iconPath = join(iconRoot, '32x32.png');
  const icon = readFileSync(iconPath);
  icon.writeUInt32BE(31, 16);
  writeFileSync(iconPath, icon);
  assert.throws(() => validateInstallerContract(root), /must be 32x32; found 31x32/);
});

test('rejects an ICO directory whose image data points outside the file', (t) => {
  const { root, iconRoot } = makeFixture(t);
  const iconPath = join(iconRoot, 'icon.ico');
  const icon = readFileSync(iconPath);
  icon.writeUInt32LE(0xffff, 18);
  writeFileSync(iconPath, icon);
  assert.throws(() => validateInstallerContract(root), /out-of-range or incomplete image entry/);
});

test('rejects packaged resources that could silently embed unverified artifacts', (t) => {
  const { root } = makeFixture(t);
  const configPath = join(root, 'apps', 'desktop', 'src-tauri', 'tauri.conf.json');
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle.resources = ['models/*.gguf'];
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
  assert.throws(() => validateInstallerContract(root), /resources must not embed/);
});
