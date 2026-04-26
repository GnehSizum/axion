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

  window.__AXION_GUI_SMOKE__ = async () => {
    const exportedAt = new Date();
    const checks = [];
    const pushCheck = (id, label, statusValue, detail) => {
      checks.push({ id, label, status: statusValue, detail });
    };
    const bridgeInfo =
      typeof diagnostics?.describeBridge === 'function' ? diagnostics.describeBridge() : null;

    pushCheck(
      'bridge.bootstrap',
      'Bridge bootstrap available',
      bridge.ready === true ? 'pass' : 'fail',
      bridge.ready === true ? bridge.version : 'window.__AXION__.ready is false',
    );
    pushCheck(
      'bridge.diagnostics',
      'Bridge diagnostics available',
      diagnostics ? 'pass' : 'fail',
      diagnostics ? 'describeBridge/snapshotTextControl/toPrettyJson present' : 'diagnostics missing',
    );
    pushCheck(
      'bridge.compat.text_input',
      'Input compat helper available',
      typeof bridge.compat?.installTextInputSelectionPatch === 'function' ? 'pass' : 'fail',
      typeof bridge.compat?.installTextInputSelectionPatch === 'function'
        ? 'installTextInputSelectionPatch'
        : 'compat helper missing',
    );

    let ping = null;
    let appInfo = null;
    let appVersion = null;
    let windowInfo = null;
    let greeting = null;

    try {
      ping = await bridge.invoke('app.ping', { from: 'hello-axion-gui-smoke' });
      pushCheck('app.ping', 'app.ping', ping?.message === 'pong' ? 'pass' : 'fail', ping?.message ?? 'missing pong');
    } catch (error) {
      pushCheck('app.ping', 'app.ping', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      appInfo = await bridge.invoke('app.info', null);
      pushCheck('app.info', 'app.info', appInfo?.appName === 'hello-axion' ? 'pass' : 'fail', appInfo?.appName ?? 'missing appName');
    } catch (error) {
      pushCheck('app.info', 'app.info', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      appVersion = await bridge.invoke('app.version', null);
      pushCheck('app.version', 'app.version', appVersion?.framework === 'axion' ? 'pass' : 'fail', appVersion?.release ?? 'missing release');
    } catch (error) {
      pushCheck('app.version', 'app.version', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      windowInfo = await bridge.invoke('window.info', null);
      pushCheck('window.info', 'window.info', windowInfo?.id === 'main' ? 'pass' : 'fail', windowInfo?.id ?? 'missing window id');
    } catch (error) {
      pushCheck('window.info', 'window.info', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      greeting = await bridge.invoke('demo.greet', { from: 'hello-axion-gui-smoke' });
      pushCheck('demo.greet', 'demo.greet', greeting?.appName === 'hello-axion' ? 'pass' : 'fail', greeting?.appName ?? 'missing appName');
    } catch (error) {
      pushCheck('demo.greet', 'demo.greet', 'fail', error instanceof Error ? error.message : String(error));
    }

    const inputSnapshot =
      typeof diagnostics?.snapshotTextControl === 'function'
        ? diagnostics.snapshotTextControl(compatInput, { source: 'gui-smoke' })
        : null;
    pushCheck(
      'input.snapshot',
      'Text control snapshot',
      inputSnapshot && inputSnapshot.targetId === 'compat-input' ? 'pass' : 'fail',
      inputSnapshot ? inputSnapshot.targetId : 'snapshotTextControl unavailable',
    );

    const result = checks.some((check) => check.status === 'fail') ? 'failed' : 'ok';
    return {
      schema: diagnostics?.reportSchema ?? bridgeInfo?.diagnosticsReportSchema ?? 'axion.diagnostics-report.v1',
      source: 'hello-axion',
      exported_at: exportedAt.toISOString(),
      exported_at_unix_seconds: Math.floor(exportedAt.getTime() / 1000),
      manifest_path: null,
      app_name: appInfo?.appName ?? bridge.appName,
      identifier: appInfo?.identifier ?? null,
      version: appInfo?.version ?? null,
      description: appInfo?.description ?? null,
      authors: Array.isArray(appInfo?.authors) ? appInfo.authors : [],
      homepage: appInfo?.homepage ?? null,
      mode: appInfo?.mode ?? 'production',
      window_count: windowInfo ? 1 : 0,
      windows: windowInfo
        ? [{
            id: windowInfo.id,
            title: windowInfo.title,
            bridge_enabled: true,
            configured_commands: [...bridge.commands],
            configured_events: [...bridge.events],
            configured_protocols: [bridge.protocol ?? 'axion'],
            runtime_command_count: bridge.commands.length,
            runtime_event_count: bridge.events.length,
            host_events: [...bridge.hostEvents],
            trusted_origins: [...bridge.trustedOrigins],
            allowed_navigation_origins: ['https://docs.example'],
            allow_remote_navigation: false,
            width: windowInfo.width,
            height: windowInfo.height,
            resizable: windowInfo.resizable,
            visible: windowInfo.visible,
            focused: windowInfo.focused,
          }]
        : [],
      frontend_dist: null,
      entry: bridgeInfo?.locationHref ?? window.location.href,
      configured_dialog_backend: null,
      dialog_backend: null,
      icon: null,
      host_events: [...bridge.hostEvents],
      staged_app_dir: null,
      asset_manifest_path: null,
      artifacts_removed: null,
      result,
      diagnostics: {
        bridge: bridgeInfo,
        app_version: appVersion,
        greeting,
        smoke_checks: checks,
        compat_input: inputSnapshot,
      },
    };
  };

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
