// Provisional platform calibration: fixed synthetic fixture, no entered values.
import { createServer } from 'node:http';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = process.cwd();
const output = join(root, '.smoogle/browser-contracts');
mkdirSync(output, { recursive: true });
const fixture = readFileSync(join(root, 'scripts/fixtures/browser-contracts/index.html'));
const server = createServer((_request, response) => {
  response.writeHead(200, { 'Content-Type': 'text/html' });
  response.end(fixture);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const evidence = {
  source_revision: spawnSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim(),
  fixture_sha256: createHash('sha256').update(fixture).digest('hex'),
  platform: process.platform,
  architecture: process.arch,
  started_at: new Date().toISOString(),
  scope: 'Calibration only: actual document focus, focus events and disabled state on a synthetic fixture.',
  arms: [],
};
const stateExpression = `({hasFocus: document.hasFocus(), visibility: document.visibilityState,
  active: document.activeElement?.id, disabled: document.querySelector('#target')?.disabled,
  readonly: document.querySelector('#target')?.readOnly, events: window.focusEvents,
  title: document.title})`;

async function arm(mode) {
  const profile = mkdtempSync(join(tmpdir(), 'lantern-focus-calibration-'));
  let browser;
  let ws;
  let stderr = '';
  const result = { mode };
  try {
    browser = spawn(process.env.LANTERN_CHROMIUM, [
      '--headless=new', '--remote-debugging-address=127.0.0.1', '--remote-debugging-port=0',
      `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check',
      '--window-size=1000,700', 'about:blank',
    ], { stdio: ['ignore', 'ignore', 'pipe'] });
    let startError;
    browser.on('error', error => { startError = error; });
    browser.stderr.on('data', data => { stderr = (stderr + data).slice(-20000); });
    for (let n = 0; n < 200 && !existsSync(join(profile, 'DevToolsActivePort')); n++) {
      if (startError) throw startError;
      if (browser.exitCode !== null) throw Error(`Chromium exited: ${browser.exitCode}`);
      await delay(50);
    }
    const port = readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0];
    const endpoint = `http://127.0.0.1:${port}`;
    const version = await (await fetch(endpoint + '/json/version', { signal: AbortSignal.timeout(3000) })).json();
    result.browser = version.Browser;
    const targets = await (await fetch(endpoint + '/json/list', { signal: AbortSignal.timeout(3000) })).json();
    ws = new WebSocket(targets.find(target => target.type === 'page').webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('WebSocket attachment timed out')), 3000);
      ws.onopen = () => { clearTimeout(timer); resolve(); };
      ws.onerror = event => { clearTimeout(timer); reject(Error(String(event))); };
    });
    let id = 0;
    const pending = new Map();
    ws.onmessage = event => {
      const response = JSON.parse(event.data);
      if (pending.has(response.id)) {
        const { resolve, reject, timer } = pending.get(response.id);
        clearTimeout(timer);
        pending.delete(response.id);
        if (response.error) reject(Error(JSON.stringify(response.error)));
        else resolve(response.result);
      }
    };
    const call = (method, params = {}) => new Promise((resolve, reject) => {
      const next = ++id;
      const timer = setTimeout(() => { pending.delete(next); reject(Error(`CDP timed out: ${method}`)); }, 3000);
      pending.set(next, { resolve, reject, timer });
      ws.send(JSON.stringify({ id: next, method, params }));
    });
    const evaluate = async expression => {
      const response = await call('Runtime.evaluate', { expression, returnByValue: true });
      if (response.exceptionDetails) throw Error('Fixture probe exception');
      return response.result.value;
    };
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/?case=focus-disables` });
    for (let n = 0; n < 100; n++) {
      if (await evaluate("document.readyState === 'complete' && !!document.querySelector('#target')")) break;
      await delay(20);
    }
    await evaluate(`window.focusEvents = []; for (const type of ['focus', 'focusin', 'blur', 'focusout'])
      document.addEventListener(type, event => focusEvents.push({type, id: event.target.id,
      hasFocus: document.hasFocus()}), true)`);
    result.before = await evaluate(stateExpression);
    if (mode === 'bring-to-front') await call('Page.bringToFront');
    if (mode === 'focus-emulation-diagnostic') await call('Emulation.setFocusEmulationEnabled', { enabled: true });
    result.after_activation = await evaluate(stateExpression);
    if (mode === 'dom-focus') {
      const document = await call('DOM.getDocument', { depth: 0 });
      const target = await call('DOM.querySelector', { nodeId: document.root.nodeId, selector: '#target' });
      await call('DOM.focus', { nodeId: target.nodeId });
    } else {
      await evaluate("document.querySelector('#target').focus({preventScroll: true})");
    }
    result.after_focus = await evaluate(stateExpression);
    await delay(150);
    result.after_settle = await evaluate(stateExpression);
  } catch (error) {
    result.error = String(error);
  } finally {
    if (ws) ws.close();
    if (browser && browser.exitCode === null) {
      browser.kill();
      for (let n = 0; n < 60 && browser.exitCode === null; n++) await delay(50);
      if (browser.exitCode === null) browser.kill('SIGKILL');
    }
    writeFileSync(join(output, `focus-${mode}.log`), stderr);
    rmSync(profile, { recursive: true, force: true });
  }
  evidence.arms.push(result);
  writeFileSync(join(output, 'focus-calibration.json'), JSON.stringify(evidence, null, 2) + '\n');
}

try {
  for (const mode of ['baseline', 'bring-to-front', 'dom-focus', 'focus-emulation-diagnostic']) await arm(mode);
} finally {
  server.close();
  evidence.finished_at = new Date().toISOString();
  writeFileSync(join(output, 'focus-calibration.json'), JSON.stringify(evidence, null, 2) + '\n');
}
console.log(JSON.stringify(evidence, null, 2));
if (evidence.arms.some(result => result.error)) process.exitCode = 1;
