const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const assets = path.resolve(__dirname, '../src/assets');
const template = fs.readFileSync(path.join(assets, 'bootstrap.js'), 'utf8')
  .replace('/* AXION_COMPAT_HELPERS */', () => fs.readFileSync(path.join(assets, 'compat.js'), 'utf8'))
  .replace('/* AXION_DIAGNOSTICS_HELPERS */', () => fs.readFileSync(path.join(assets, 'diagnostics.js'), 'utf8'));

function configuration(overrides = {}) {
  return {
    appName: 'test-app', bridgeToken: 'test-token', commands: ['app.ping'],
    events: ['app.log'], hostEvents: ['app.ready'], trustedOrigins: ['axion://app'],
    protocol: 'axion', version: 'v0.6.2-bootstrap',
    diagnosticsReportSchema: 'axion.diagnostics-report.v1', ...overrides
  };
}

function fixture(config = configuration(), href = 'axion://app/index.html') {
  const requests = [];
  const events = [];
  const window = { location: { href }, dispatchEvent(event) { events.push(event); return true; } };
  const script = template.replace('/* AXION_CONFIG */ null', () => JSON.stringify(config));
  const context = vm.createContext({
    window, URL, console,
    CustomEvent: class { constructor(type, init) { this.type = type; this.detail = init.detail; } },
    async fetch(url, options) {
      requests.push({ url, options });
      const request = new URL(url);
      return { ok: true, async json() { return { ok: true, id: request.searchParams.get('id'), payload: 'pong' }; } };
    }
  });
  vm.runInContext(script, context);
  return { bridge: window.__AXION__, requests, events, window, script, context };
}

test('configuration strings and marker-like data remain data', () => {
  const appName = 'quoted "name"; /* AXION_COMPAT_HELPERS */\n\0\u2028中文';
  const bridgeToken = '/* AXION_CONFIG */ null /* AXION_DIAGNOSTICS_HELPERS */ $&';
  const { bridge, context } = fixture(configuration({ appName, bridgeToken }));
  assert.equal(bridge.appName, appName);
  assert.equal(bridge.diagnostics.describeBridge().appName, appName);
  assert.equal(context.injected, undefined);
  assert.ok(Object.isFrozen(bridge));
  assert.ok(Object.isFrozen(bridge.commands));
  assert.ok(Object.isFrozen(bridge.trustedOrigins));
});

test('untrusted navigation does not receive bootstrap', () => {
  assert.equal(fixture(configuration(), 'https://remote.example/index.html').bridge, undefined);
});

test('invoke preserves request id, payload and the bearer header', async () => {
  const { bridge, requests } = fixture();
  assert.equal(await bridge.invoke('app.ping', { text: '中文 "quoted"' }), 'pong');
  const request = new URL(requests[0].url);
  assert.equal(request.pathname, '/__axion__/invoke/app.ping');
  assert.deepEqual(JSON.parse(request.searchParams.get('payload')), { text: '中文 "quoted"' });
  assert.match(request.searchParams.get('id'), /^axion_/);
  assert.equal(requests[0].options.headers['X-Axion-Bridge-Token'], 'test-token');
  await assert.rejects(bridge.invoke('fs.read_text', {}), (error) => error.code === 'bridge.command-not-allowed');
  assert.equal(requests.length, 1);
});

test('host events require the exact token and an allowed host event', () => {
  const { bridge, events } = fixture();
  const received = [];
  const dispose = bridge.listen('app.ready', (payload) => received.push(payload));
  assert.equal(bridge.__dispatchFromHost('wrong-token', 'app.ready', { ready: false }), false);
  assert.equal(bridge.__dispatchFromHost('test-token', 'unknown.event', {}), false);
  assert.equal(bridge.__dispatchFromHost('test-token', 'app.ready', { ready: true }), true);
  assert.equal(received.length, 1);
  assert.equal(received[0].ready, true);
  assert.equal(events[0].type, 'axion:app.ready');
  dispose();
  bridge.__dispatchFromHost('test-token', 'app.ready', { ready: false });
  assert.equal(received.length, 1);
});

test('diagnostics preserve structured and legacy error messages', () => {
  const { bridge } = fixture();
  const legacy = bridge.diagnostics.normalizeError('fs.not-found: missing');
  assert.equal(legacy.code, 'fs.not-found');
  assert.equal(legacy.message, 'fs.not-found: missing');
  const structured = bridge.diagnostics.normalizeError({ code: 'bridge.busy', message: 'queue is full' });
  assert.equal(structured.code, 'bridge.busy');
  assert.equal(structured.message, 'queue is full');
});

test('installing again preserves the first bridge instance', () => {
  const { bridge, window, script, context } = fixture();
  vm.runInContext(script, context);
  assert.equal(window.__AXION__, bridge);
});
