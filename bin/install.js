#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync, spawn } = require('child_process');
const readline = require('readline');

const REPO_OWNER = 'pdxxxx';
const REPO_NAME = 'grok-search-mcp-rust';
const BINARY_NAME = 'grok-search-mcp';

const PLATFORMS = {
  'linux-x64': 'grok-search-mcp-linux-amd64',
  'linux-arm64': 'grok-search-mcp-linux-arm64',
  'darwin-x64': 'grok-search-mcp-macos-amd64',
  'darwin-arm64': 'grok-search-mcp-macos-arm64',
  'win32-x64': 'grok-search-mcp-windows-amd64.exe',
};

function getPlatformKey() {
  return `${process.platform}-${process.arch}`;
}

function getBinaryName() {
  const key = getPlatformKey();
  return PLATFORMS[key];
}

function getInstallDir() {
  if (process.platform === 'win32') {
    return path.join(process.env.LOCALAPPDATA || path.join(require('os').homedir(), 'AppData', 'Local'), BINARY_NAME);
  }
  return path.join(require('os').homedir(), '.local', 'bin');
}

function getInstallPath() {
  const dir = getInstallDir();
  const ext = process.platform === 'win32' ? '.exe' : '';
  return path.join(dir, BINARY_NAME + ext);
}

function getConfigDir() {
  if (process.platform === 'darwin') {
    return path.join(require('os').homedir(), 'Library', 'Application Support', 'grok-search');
  }
  if (process.platform === 'win32') {
    return path.join(process.env.APPDATA || path.join(require('os').homedir(), 'AppData', 'Roaming'), 'grok-search');
  }
  return path.join(require('os').homedir(), '.config', 'grok-search');
}

async function fetchJson(url) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers: { 'User-Agent': 'grok-search-mcp-installer' } }, (res) => {
      if (res.statusCode === 302 || res.statusCode === 301) {
        return fetchJson(res.headers.location).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`HTTP ${res.statusCode}`));
      }
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try { resolve(JSON.parse(data)); }
        catch (e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.setTimeout(60000, () => { req.destroy(); reject(new Error('Timeout')); });
  });
}

async function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(dest);
    const request = (url) => {
      https.get(url, { headers: { 'User-Agent': 'grok-search-mcp-installer' } }, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          return request(res.headers.location);
        }
        if (res.statusCode !== 200) {
          file.close();
          fs.unlinkSync(dest);
          return reject(new Error(`HTTP ${res.statusCode}`));
        }
        res.pipe(file);
        file.on('finish', () => { file.close(); resolve(); });
      }).on('error', (e) => {
        file.close();
        fs.unlinkSync(dest);
        reject(e);
      });
    };
    request(url);
  });
}

async function getLatestRelease() {
  const url = `https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest`;
  return fetchJson(url);
}

async function getInstalledVersion() {
  const binPath = getInstallPath();
  if (!fs.existsSync(binPath)) return null;
  try {
    const output = execSync(`"${binPath}" --version`, { encoding: 'utf8', timeout: 5000 });
    const match = output.match(/(\d+\.\d+\.\d+)/);
    return match ? `v${match[1]}` : null;
  } catch { return null; }
}

function prompt(question) {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise(resolve => rl.question(question, ans => { rl.close(); resolve(ans); }));
}

async function install() {
  const binaryName = getBinaryName();
  if (!binaryName) {
    console.error(`Unsupported platform: ${getPlatformKey()}`);
    console.error(`Supported: ${Object.keys(PLATFORMS).join(', ')}`);
    process.exit(1);
  }

  console.log('Fetching latest release...');
  const release = await getLatestRelease();
  const asset = release.assets.find(a => a.name === binaryName);
  if (!asset) {
    console.error(`Binary not found for platform: ${binaryName}`);
    process.exit(1);
  }

  const installDir = getInstallDir();
  const installPath = getInstallPath();

  if (fs.existsSync(installPath)) {
    const ans = await prompt('grok-search-mcp already installed. Overwrite? [y/N] ');
    if (ans.toLowerCase() !== 'y') {
      console.log('Installation cancelled.');
      return;
    }
  }

  fs.mkdirSync(installDir, { recursive: true });
  console.log(`Downloading ${asset.name}...`);
  await downloadFile(asset.browser_download_url, installPath);

  if (process.platform !== 'win32') {
    fs.chmodSync(installPath, 0o755);
  }

  console.log(`\n✅ Installed to: ${installPath}`);
  console.log(`   Version: ${release.tag_name}`);

  if (process.platform !== 'win32') {
    const pathEnv = process.env.PATH || '';
    if (!pathEnv.includes(installDir)) {
      console.log(`\n⚠️  Note: Add ${installDir} to your PATH to run grok-search-mcp directly`);
    }
  }
}

async function update() {
  const installed = await getInstalledVersion();
  if (!installed) {
    console.log('grok-search-mcp is not installed. Use option 1 to install.');
    return;
  }

  console.log('Checking for updates...');
  const release = await getLatestRelease();
  const latest = release.tag_name;

  if (installed === latest) {
    console.log(`Already up to date (${latest})`);
    return;
  }

  console.log(`Update available: ${installed} → ${latest}`);
  const ans = await prompt('Proceed with update? [Y/n] ');
  if (ans.toLowerCase() === 'n') return;

  await install();
}

async function checkUpdates() {
  const installed = await getInstalledVersion();
  if (!installed) {
    console.log('grok-search-mcp is not installed. Use option 1 to install.');
    return;
  }

  console.log('Checking for updates...');
  const release = await getLatestRelease();
  const latest = release.tag_name;

  if (installed === latest) {
    console.log(`✅ Already up to date (${latest})`);
  } else {
    console.log(`⬆️  Update available: ${installed} → ${latest}`);
  }
}

async function configureClaude() {
  const binPath = getInstallPath();
  if (!fs.existsSync(binPath)) {
    console.log('grok-search-mcp is not installed. Please install first.');
    return;
  }

  let claudeExists = false;
  try {
    execSync('claude --version', { stdio: 'ignore' });
    claudeExists = true;
  } catch {}

  if (!claudeExists) {
    console.log('\nClaude CLI not found. Please install Claude Code first, then run:');
    console.log(`  claude mcp add grok-search -s user --transport stdio -- "${binPath}"`);
    return;
  }

  const apiUrl = await prompt('Enter GROK_API_URL: ');
  const apiKey = await prompt('Enter GROK_API_KEY: ');

  if (!apiUrl || !apiKey) {
    console.log('API URL and Key are required.');
    return;
  }

  try {
    execSync('claude mcp remove grok-search -s user', { stdio: 'ignore' });
  } catch {}

  const config = {
    type: 'stdio',
    command: binPath,
    env: { GROK_API_URL: apiUrl, GROK_API_KEY: apiKey }
  };

  try {
    execSync(`claude mcp add-json grok-search --scope user '${JSON.stringify(config)}'`, { stdio: 'inherit' });
    console.log('\n✅ Claude Code configured successfully!');
  } catch (e) {
    console.error('Failed to configure Claude Code:', e.message);
  }
}

async function uninstall() {
  const binPath = getInstallPath();
  if (!fs.existsSync(binPath)) {
    console.log('grok-search-mcp is not installed.');
    return;
  }

  try {
    execSync('claude mcp remove grok-search -s user', { stdio: 'ignore' });
  } catch {}

  fs.unlinkSync(binPath);
  console.log('✅ Binary removed.');

  const ans = await prompt('Also remove configuration files? [y/N] ');
  if (ans.toLowerCase() === 'y') {
    const configDir = getConfigDir();
    if (fs.existsSync(configDir)) {
      fs.rmSync(configDir, { recursive: true });
      console.log('✅ Configuration removed.');
    }
  }

  console.log('✅ grok-search-mcp uninstalled successfully.');
}

async function main() {
  console.log('╔════════════════════════════════════════╗');
  console.log('║   Grok Search MCP Installer            ║');
  console.log('╚════════════════════════════════════════╝\n');

  const args = process.argv.slice(2);
  if (args.includes('--install')) return install();
  if (args.includes('--update')) return update();
  if (args.includes('--uninstall')) return uninstall();

  if (!process.stdin.isTTY) {
    console.log('Non-interactive mode. Use --install, --update, or --uninstall flags.');
    process.exit(1);
  }

  console.log('1. Install');
  console.log('2. Update');
  console.log('3. Check for updates');
  console.log('4. Configure Claude Code');
  console.log('5. Uninstall');
  console.log('');

  const choice = await prompt('Select option [1-5]: ');

  switch (choice.trim()) {
    case '1': await install(); break;
    case '2': await update(); break;
    case '3': await checkUpdates(); break;
    case '4': await configureClaude(); break;
    case '5': await uninstall(); break;
    default: console.log('Invalid option.');
  }
}

main().catch(e => { console.error('Error:', e.message); process.exit(1); });
