// Test the exact release-please version bundled by the pinned action, without GitHub writes.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
require('release-please'); // Initialize the library's cyclic strategy registry.
const {buildStrategy} = require('release-please/build/src/factory');
const {parseConventionalCommits} = require('release-please/build/src/commit');
const {Version} = require('release-please/build/src/version');
const {TagName} = require('release-please/build/src/util/tag-name');
const root = path.resolve(__dirname, '../..');
const manifest = JSON.parse(fs.readFileSync(path.join(root, '.release-please-manifest.json')));
assert(Object.keys(manifest).every(key => key === '.'));
const fullConfig = JSON.parse(fs.readFileSync(path.join(root, 'release-please-config.json')));
assert.equal(fullConfig['bootstrap-sha'], undefined, 'keep initial implementation history');
const config = JSON.parse(fs.readFileSync(path.join(root, 'release-please-config.json'))).packages['.'];
assert.equal(require('release-please/package.json').version, '17.1.3');
const files = new Map();
for (const file of ['Cargo.toml', 'Cargo.lock', 'desktop/package.json', 'desktop/src-tauri/tauri.conf.json', 'version.txt']) {
  files.set(file, fs.readFileSync(path.join(root, file), 'utf8'));
}
const initialRoot = files.get('Cargo.toml');
assert(!initialRoot.includes('x-release-please-version'), 'root updater must work without markers');
const initialLock = files.get('Cargo.lock');
const packageBlock = /(?<=^|\n)\[\[package\]\]\n[\s\S]*?(?=\n\[\[package\]\]|$)/g;
const github = {
  repository: {owner: 'fixture', repo: 'll-control'},
  async getFileContentsOnBranch(file) {
    if (!files.has(file)) { const error = new Error('missing fixture'); error.status = 404; throw error; }
    return {parsedContent: files.get(file), content: '', sha: 'fixture'};
  },
};
async function run() {
  const strategy = await buildStrategy({releaseType: config['release-type'], github, targetBranch: 'master', packageName: config['package-name'],
  initialVersion: config['initial-version'], includeComponentInTag: false,
  includeVInTag: true, bumpMinorPreMajor: config['bump-minor-pre-major'],
  bumpPatchForMinorPreMajor: config['bump-patch-for-minor-pre-major'], extraFiles: config['extra-files']});

  let latest;
  for (const [index, [message, expected]] of [
    ['feat: initial implemented product', '0.1.0'],
    ['fix: repair import', '0.1.1'],
    ['feat: add capability', '0.2.0'],
    ['feat!: change protocol', '0.3.0'],
  ].entries()) {
    const sha = String(index + 1).repeat(40);
    const proposal = await strategy.buildReleasePullRequest(parseConventionalCommits([{sha, message, files: ['Cargo.toml']}]), latest);
    assert(proposal, 'release candidate exists');
    assert.equal(proposal.version.toString(), expected);
    for (const update of proposal.updates) {
      files.set(update.path, update.updater.updateContent(files.get(update.path)));
    }
    const expectedRoot = initialRoot.replace(/(\[workspace\.package\][\s\S]*?version\s*=\s*")[^"]+"/, `$1${expected}"`);
    assert.equal(files.get('Cargo.toml'), expectedRoot, 'only workspace product version changes');
    const expectedLock = initialLock.replace(packageBlock, block =>
      /^source = /m.test(block) ? block : block.replace(/^version = "[^"]+"$/m, `version = "${expected}"`));
    assert.equal(files.get('Cargo.lock'), expectedLock, 'all local entries update; third-party bytes stay identical');
    assert.equal(JSON.parse(files.get('desktop/package.json')).version, expected);
    assert.equal(JSON.parse(files.get('desktop/src-tauri/tauri.conf.json')).version, expected);
    assert.equal(files.get('version.txt').trim(), expected);
    const fixtureRoot = process.env.RELEASE_FIXTURE_DIR || path.join(os.tmpdir(), 'll-release-fixtures');
    const temp = path.join(fixtureRoot, expected);
    fs.mkdirSync(temp, {recursive: true});
    const memberList = files.get('Cargo.toml').match(/members\s*=\s*\[([\s\S]*?)\]/)[1];
    for (const match of memberList.matchAll(/"([^"]+)"/g)) {
      const member = match[1];
      fs.mkdirSync(path.join(temp, member), {recursive: true});
      fs.copyFileSync(path.join(root, member, 'Cargo.toml'), path.join(temp, member, 'Cargo.toml'));
    }
    for (const [file, content] of files) {
      if (file === 'CHANGELOG.md') continue;
      fs.mkdirSync(path.dirname(path.join(temp, file)), {recursive: true});
      fs.writeFileSync(path.join(temp, file), content);
    }
    latest = {name: `v${expected}`, tag: new TagName(Version.parse(expected), undefined, '-', true), sha, notes: 'fixture'};
    console.log(`${message} -> v${expected}: all product/lock versions updated; third-party bytes preserved`);
  }
}
run().catch(error => {console.error(error); process.exitCode = 1;});
