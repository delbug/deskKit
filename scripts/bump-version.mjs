#!/usr/bin/env node
/**
 * 根据最新 git tag（或 package.json）递增版本号，并同步到各配置文件。
 * 用法: node scripts/bump-version.mjs [patch|minor|major]
 */
import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(__dirname, '..');

const bumpType = (process.argv[2] || 'patch').toLowerCase();
if (!['patch', 'minor', 'major'].includes(bumpType)) {
  console.error('用法: node scripts/bump-version.mjs [patch|minor|major]');
  process.exit(1);
}

function parseVersion(v) {
  const m = String(v).trim().match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!m) throw new Error(`无效版本号: ${v}`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function formatVersion(parts) {
  return parts.join('.');
}

function compareSemver(a, b) {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  for (let i = 0; i < 3; i += 1) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

function bumpVersion(version, type) {
  const parts = parseVersion(version);
  if (type === 'major') {
    parts[0] += 1;
    parts[1] = 0;
    parts[2] = 0;
  } else if (type === 'minor') {
    parts[1] += 1;
    parts[2] = 0;
  } else {
    parts[2] += 1;
  }
  return formatVersion(parts);
}

function readPackageVersion() {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  return pkg.version;
}

function readLatestTagVersion() {
  try {
    const out = execSync("git tag -l 'v*' --sort=-v:refname", {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
    const first = out.split('\n').find(Boolean);
    if (!first) return null;
    return first.replace(/^v/i, '');
  } catch {
    return null;
  }
}

function currentBaseVersion() {
  const fromPkg = readPackageVersion();
  const fromTag = readLatestTagVersion();
  if (!fromTag) return fromPkg;
  return compareSemver(fromTag, fromPkg) >= 0 ? fromTag : fromPkg;
}

function updateJsonFile(filePath, version) {
  const data = JSON.parse(fs.readFileSync(filePath, 'utf8'));
  data.version = version;
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`);
}

function updateCargoToml(version) {
  const filePath = path.join(root, 'src-tauri/Cargo.toml');
  const content = fs.readFileSync(filePath, 'utf8');
  const next = content.replace(/^version = "[^"]+"/m, `version = "${version}"`);
  fs.writeFileSync(filePath, next);
}

function updateCargoLock(version) {
  const filePath = path.join(root, 'src-tauri/Cargo.lock');
  let content = fs.readFileSync(filePath, 'utf8');
  content = content.replace(
    /(\[\[package\]\]\nname = "deskkit"\nversion = ")[^"]+(")/,
    `$1${version}$2`,
  );
  fs.writeFileSync(filePath, content);
}

const base = currentBaseVersion();
const next = bumpVersion(base, bumpType);

updateJsonFile(path.join(root, 'package.json'), next);
updateJsonFile(path.join(root, 'src-tauri/tauri.conf.json'), next);
updateCargoToml(next);
updateCargoLock(next);

if (process.env.GITHUB_OUTPUT) {
  fs.appendFileSync(process.env.GITHUB_OUTPUT, `version=${next}\n`);
  fs.appendFileSync(process.env.GITHUB_OUTPUT, `tag=v${next}\n`);
}

console.log(next);
