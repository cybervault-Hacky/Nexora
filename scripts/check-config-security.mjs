import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const envTemplatePath = join(repositoryRoot, '.env.example');
const builtFrontendPath = join(repositoryRoot, 'apps', 'desktop', 'ui', 'dist');
const failures = [];

const placeholderValue = /(REPLACE_WITH|CHANGE_ME|CHANGEME|PLACEHOLDER|EXAMPLE_ONLY|NEXORA\.LOCAL)/i;
const secretSettingName = /(SECRET|PASSWORD|PASSWD|DATABASE_URL|WEBHOOK|PRIVATE_KEY|API_KEY|ACCESS_TOKEN|REFRESH_TOKEN)/i;
const publicOAuthClientIdName = /^(GOOGLE_OAUTH_CLIENT_ID|NEXORA_AUTH_GOOGLE_CLIENT_ID)$/;

function report(path, label) {
  // Never print a matched value: diagnostics identify only the path and rule.
  failures.push(`${relative(repositoryRoot, path)}: ${label}`);
}

function readTemplateAssignments() {
  const contents = readFileSync(envTemplatePath, 'utf8');
  const entries = new Map();

  for (const [index, line] of contents.split(/\r?\n/).entries()) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const separator = trimmed.indexOf('=');
    if (separator < 1) {
      report(envTemplatePath, `malformed assignment on line ${index + 1}`);
      continue;
    }
    const key = trimmed.slice(0, separator).trim();
    const value = trimmed.slice(separator + 1).trim();
    entries.set(key, value);

    if (key.startsWith('VITE_')) {
      report(envTemplatePath, `backend setting ${key} must not use the VITE_ prefix`);
    }
    if (value && secretSettingName.test(key) && !placeholderValue.test(value)) {
      report(envTemplatePath, `secret-bearing setting ${key} must be blank or a clear placeholder`);
    }
    if (value && publicOAuthClientIdName.test(key) && !placeholderValue.test(value)) {
      report(envTemplatePath, `OAuth client ID ${key} must be a placeholder in the template`);
    }
  }

  return entries;
}

function collectTextFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectTextFiles(path));
    else if (/\.(?:html|js|css|json|map)$/i.test(entry.name)) files.push(path);
  }
  return files;
}

function scanBuiltFrontend() {
  if (!existsSync(builtFrontendPath)) {
    console.log('Config template check passed; frontend bundle scan skipped (no build output found).');
    return;
  }

  const forbiddenNames = [
    'NEXORA_AUTH_DATABASE_URL',
    'NEXORA_AUTH_TEST_DATABASE_URL',
    'NEXORA_AUTH_GOOGLE_CLIENT_SECRET',
    'GOOGLE_OAUTH_CLIENT_SECRET',
    'NEXORA_AUTH_SMTP_PASSWORD',
    'STRIPE_SECRET_KEY',
    'STRIPE_WEBHOOK_SECRET',
    'RAZORPAY_KEY_SECRET',
    'RAZORPAY_WEBHOOK_SECRET',
  ];
  const secretPatterns = [
    ['Stripe secret-key pattern', /\bsk_(?:live|test)_[A-Za-z0-9]{8,}\b/],
    ['Stripe webhook-secret pattern', /\bwhsec_[A-Za-z0-9]{8,}\b/],
    ['Razorpay secret-key pattern', /\brzp_(?:live|test)_[A-Za-z0-9]{8,}\b/],
    ['database URL with user-info', /\bpostgres(?:ql)?:\/\/[^\s/@:]+:[^\s/@]+@/i],
    ['private-key PEM marker', /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/],
    ['Google API-key pattern', /\bAIza[0-9A-Za-z_-]{35}\b/],
  ];

  for (const path of collectTextFiles(builtFrontendPath)) {
    const content = readFileSync(path, 'utf8');
    for (const name of forbiddenNames) {
      if (content.includes(name)) report(path, `backend-only setting name ${name} found in frontend output`);
    }
    for (const [label, pattern] of secretPatterns) {
      if (pattern.test(content)) report(path, `${label} found in frontend output`);
    }
  }

  // Also catch accidentally tracked dotenv files without opening or reading them.
  try {
    const tracked = execFileSync('git', ['ls-files', '-z'], { cwd: repositoryRoot });
    for (const path of tracked.toString('utf8').split('\0').filter(Boolean)) {
      if (/(?:^|\/)\.env(?:\..+)?$/.test(path) && path !== '.env.example') {
        report(join(repositoryRoot, path), 'dotenv file is tracked; only .env.example may be committed');
      }
    }
  } catch {
    // Source archives without Git metadata can still run template/bundle checks.
  }
}

if (!existsSync(envTemplatePath)) {
  console.error('Config security check failed: .env.example is missing.');
  process.exit(1);
}

readTemplateAssignments();
scanBuiltFrontend();

if (failures.length > 0) {
  console.error('Config security check failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('Config security checks passed; no secret values were printed.');
