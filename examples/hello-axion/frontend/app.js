window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
  const compatInput = document.getElementById('compat-input');
  const compatTextarea = document.getElementById('compat-textarea');
  const compatDiagnostics = document.getElementById('compat-diagnostics');
  if (!status) return;
  const bridge = window.__AXION__;
  const diagnostics = bridge?.diagnostics;
  const formatPretty = (value) =>
    typeof diagnostics?.toPrettyJson === 'function'
      ? diagnostics.toPrettyJson(value)
      : JSON.stringify(value, null, 2);

  const renderCompatDiagnostics = (value) => {
    if (compatDiagnostics) {
      compatDiagnostics.textContent = formatPretty(value);
    }
  };

  if (!bridge) {
    status.textContent = 'Axion bootstrap was not injected.';
    if (details) details.textContent = 'Bridge unavailable';
    renderCompatDiagnostics({ error: 'Bridge unavailable' });
    return;
  }

  const installTextInputSelectionPatch =
    bridge.compat?.installTextInputSelectionPatch;

  if (typeof installTextInputSelectionPatch === 'function') {
    const patchTargets = [
      [compatInput, false],
      [compatTextarea, true],
    ];
    for (const [element, manualPointerSelection] of patchTargets) {
      if (!element) continue;
      installTextInputSelectionPatch(element, {
        manualPointerSelection,
        onStatus(message) {
          status.textContent = message;
        },
        onUpdate(snapshot) {
          renderCompatDiagnostics(snapshot);
        },
      });
    }
  } else {
    renderCompatDiagnostics({
      error: 'window.__AXION__.compat.installTextInputSelectionPatch is unavailable',
    });
  }

  if (compatTextarea) {
    compatTextarea.addEventListener('keydown', (event) => {
      if (event.key !== 'Tab') return;
      event.preventDefault();
      const start = compatTextarea.selectionStart ?? compatTextarea.value.length;
      const end = compatTextarea.selectionEnd ?? start;
      const value = compatTextarea.value;
      compatTextarea.value = `${value.slice(0, start)}\t${value.slice(end)}`;
      compatTextarea.selectionStart = start + 1;
      compatTextarea.selectionEnd = start + 1;
      renderCompatDiagnostics({
        targetId: compatTextarea.id,
        selectionStart: compatTextarea.selectionStart,
        selectionEnd: compatTextarea.selectionEnd,
        valueLength: compatTextarea.value.length,
        detail: { source: 'textarea-tab-handler' },
      });
    });
  }

  try {
    const lifecycleEvents = [];
    const recordLifecycleEvent = (name) => (payload) => {
      lifecycleEvents.push({ name, payload });
      if (details) {
        details.textContent = formatPretty({ lifecycleEvents });
      }
    };
    [
      'window.created',
      'window.resized',
      'window.close_requested',
      'window.closed',
      'window.redraw_failed',
    ].forEach((name) => window.__AXION__.listen(name, recordLifecycleEvent(name)));

    const pluginReady = new Promise((resolve) => {
      bridge.listen('demo.ready', resolve);
    });
    const [ping, appInfo, appVersion, appEcho, windowInfo, greeting, pluginEvent] = await Promise.all([
      bridge.invoke('app.ping', { from: 'hello-axion' }),
      bridge.invoke('app.info', null),
      bridge.invoke('app.version', null),
      bridge.invoke('app.echo', { from: 'hello-axion', async: true }),
      bridge.invoke('window.info', null),
      bridge.invoke('demo.greet', { from: 'hello-axion-plugin-demo' }),
      pluginReady,
    ]);
    const windowTitleUpdate = await bridge.invoke('window.set_title', {
      title: `${windowInfo.title} · Axion`,
    });
    const windowSizeUpdate = await bridge.invoke('window.set_size', {
      width: Math.max(windowInfo.width, 960),
      height: Math.max(windowInfo.height, 720),
    });
    const fsWrite = await bridge.invoke('fs.write_text', {
      path: 'notes/hello.txt',
      contents: 'hello-axion wrote this through the Axion bridge',
    });
    const fsRead = await bridge.invoke('fs.read_text', { path: 'notes/hello.txt' });
    const dialogOpen = await bridge.invoke('dialog.open', {
      title: 'Select files for the Axion preview',
      multiple: true,
      filters: [
        { name: 'Text', extensions: ['txt', 'md'] },
        { name: 'Images', extensions: ['png', 'jpg'] },
      ],
    });
    const dialogSave = await bridge.invoke('dialog.save', {
      title: 'Choose a save path for the Axion preview',
      defaultPath: 'notes/export.txt',
    });
    const hostLog = bridge.events.includes('app.log')
      ? await bridge.emit('app.log', {
          message: 'hello-axion frontend is ready',
          windowId: windowInfo.id,
        })
      : false;

    status.textContent = `Axion bridge ready: ${ping.message} from ${ping.appName}; plugin=${greeting.appName}`;
    if (details) {
      details.textContent = formatPretty(
        {
          bridge: typeof diagnostics?.describeBridge === 'function' ? diagnostics.describeBridge() : null,
          appInfo,
          appVersion,
          appEcho,
          windowInfo,
          windowTitleUpdate,
          windowSizeUpdate,
          greeting,
          pluginEvent,
          fsWrite,
          fsRead,
          dialogOpen,
          dialogSave,
          hostLog,
          lifecycleEvents,
        },
        null,
        2,
      );
    }
  } catch (error) {
    status.textContent = `Axion invoke failed: ${error instanceof Error ? error.message : String(error)}`;
    if (details) {
      details.textContent = status.textContent;
    }
  }
});
