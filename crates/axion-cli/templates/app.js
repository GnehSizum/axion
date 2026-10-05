window.addEventListener('DOMContentLoaded', async () => {
  const appName = '@APP_NAME@';
  const status = document.getElementById('bridge-status');
  const compatInput = document.getElementById('compat-input');
  const compatTextarea = document.getElementById('compat-textarea');
  const compatDiagnostics = document.getElementById('compat-diagnostics');
  const runNativeApiChecks = document.getElementById('run-native-api-checks');

  function renderList(id, values) {
    const list = document.getElementById(id);
    list.textContent = '';
    for (const value of values) {
      const item = document.createElement('li');
      item.textContent = value;
      list.appendChild(item);
    }
  }

  function renderJson(id, value) {
    document.getElementById(id).textContent = formatPretty(value);
  }

  function renderText(id, value) {
    document.getElementById(id).textContent = value;
  }

  function renderCompatDiagnostics(value) {
    if (compatDiagnostics) {
      compatDiagnostics.textContent = formatPretty(value);
    }
  }

  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected. Run with `cargo run --features servo-runtime`.';
    for (const id of ['app-info', 'native-api', 'custom-command', 'capability-denial', 'event-log', 'compat-diagnostics']) {
      renderText(id, 'Bridge unavailable');
    }
    return;
  }

  const axion = window.__AXION__;
  const diagnostics = axion.diagnostics;
  const formatPretty = (value) =>
    typeof diagnostics?.toPrettyJson === 'function'
      ? diagnostics.toPrettyJson(value)
      : JSON.stringify(value, null, 2);
  const normalizeError = (error) =>
    typeof diagnostics?.normalizeError === 'function'
      ? diagnostics.normalizeError(error)
      : {
          code: null,
          message: error instanceof Error ? error.message : String(error),
          cause: error ?? null,
        };
  const installTextInputSelectionPatch = axion.compat?.installTextInputSelectionPatch;
  const hostEventLog = [];
  renderList('command-list', axion.commands);
  renderList('event-list', axion.events);
  renderList('host-event-list', axion.hostEvents);

  function renderHostEventLog(extra = {}) {
    renderJson('event-log', {
      ...extra,
      frontendEvents: axion.events,
      hostEvents: axion.hostEvents,
      hostEventLog,
    });
  }

  for (const name of axion.hostEvents) {
    axion.listen(name, (payload) => {
      hostEventLog.unshift({ name, payload, receivedAt: new Date().toISOString() });
      hostEventLog.splice(8);
      renderHostEventLog();
    });
  }

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
        ...(typeof diagnostics?.snapshotTextControl === 'function'
          ? diagnostics.snapshotTextControl(compatTextarea, { source: 'textarea-tab-handler' })
          : {
              targetId: compatTextarea.id,
              selectionStart: compatTextarea.selectionStart,
              selectionEnd: compatTextarea.selectionEnd,
              valueLength: compatTextarea.value.length,
              detail: { source: 'textarea-tab-handler' },
            }),
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
      axion.ready === true ? 'pass' : 'fail',
      axion.ready === true ? axion.version : 'window.__AXION__.ready is false',
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
      typeof axion.compat?.installTextInputSelectionPatch === 'function' ? 'pass' : 'fail',
      typeof axion.compat?.installTextInputSelectionPatch === 'function'
        ? 'installTextInputSelectionPatch'
        : 'compat helper missing',
    );
    pushCheck(
      'window.lifecycle.ready',
      'window.ready host event exposed',
      axion.hostEvents.includes('window.ready') ? 'pass' : 'fail',
      axion.hostEvents.includes('window.ready') ? 'window.ready' : 'missing window.ready',
    );
    pushCheck(
      'app.exit.available',
      'app.exit capability exposed',
      axion.commands.includes('app.exit') ? 'pass' : 'fail',
      axion.commands.includes('app.exit') ? 'app.exit' : 'missing app.exit',
    );
    pushCheck(
      'window.close.available',
      'window.close capability exposed',
      axion.commands.includes('window.close') ? 'pass' : 'fail',
      axion.commands.includes('window.close') ? 'window.close' : 'missing window.close',
    );
    pushCheck(
      'shell.open.available',
      'shell.open capability exposed',
      axion.commands.includes('shell.open') ? 'pass' : 'fail',
      axion.commands.includes('shell.open') ? 'shell.open' : 'missing shell.open',
    );
    pushCheck(
      'window.close_decision.available',
      'window close decision capabilities exposed',
      axion.commands.includes('window.confirm_close') &&
        axion.commands.includes('window.prevent_close')
        ? 'pass'
        : 'fail',
      'window.confirm_close/window.prevent_close',
    );

    let ping = null;
    let appInfo = null;
    let appVersion = null;
    let windowInfo = null;
    let greeting = null;
    let clipboardRead = null;
    let fsSummary = null;

    try {
      ping = await axion.invoke('app.ping', { from: `${appName}-gui-smoke` });
      pushCheck('app.ping', 'app.ping', ping?.message === 'pong' ? 'pass' : 'fail', ping?.message ?? 'missing pong');
    } catch (error) {
      pushCheck('app.ping', 'app.ping', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      appInfo = await axion.invoke('app.info', null);
      pushCheck('app.info', 'app.info', appInfo?.appName === appName ? 'pass' : 'fail', appInfo?.appName ?? 'missing appName');
    } catch (error) {
      pushCheck('app.info', 'app.info', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      appVersion = await axion.invoke('app.version', null);
      pushCheck('app.version', 'app.version', appVersion?.framework === 'axion' ? 'pass' : 'fail', appVersion?.release ?? 'missing release');
    } catch (error) {
      pushCheck('app.version', 'app.version', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      windowInfo = await axion.invoke('window.info', null);
      pushCheck('window.info', 'window.info', windowInfo?.id ? 'pass' : 'fail', windowInfo?.id ?? 'missing window id');
    } catch (error) {
      pushCheck('window.info', 'window.info', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      greeting = await axion.invoke('demo.greet', { from: `${appName}-gui-smoke` });
      pushCheck('demo.greet', 'demo.greet', greeting?.message ? 'pass' : 'fail', greeting?.message ?? 'missing greeting');
    } catch (error) {
      pushCheck('demo.greet', 'demo.greet', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      await axion.invoke('clipboard.write_text', { text: `${appName} clipboard smoke` });
      clipboardRead = await axion.invoke('clipboard.read_text', null);
      pushCheck(
        'clipboard.roundtrip',
        'clipboard roundtrip',
        clipboardRead?.text === `${appName} clipboard smoke` ? 'pass' : 'fail',
        clipboardRead?.backend ?? 'missing backend',
      );
    } catch (error) {
      pushCheck('clipboard.roundtrip', 'clipboard roundtrip', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      const clipboardPayloadError = await axion.invoke('clipboard.write_text', {})
        .then(() => 'unexpected success')
        .catch(normalizeError);
      const dialogPayloadError = await axion.invoke('dialog.save', { multiple: true })
        .then(() => 'unexpected success')
        .catch(normalizeError);
      const windowPayloadError = await axion.invoke('window.set_size', { width: 0, height: 480 })
        .then(() => 'unexpected success')
        .catch(normalizeError);
      const shellPayloadError = await axion.invoke('shell.open', {})
        .then(() => 'unexpected success')
        .catch(normalizeError);
      pushCheck(
        'native.expected_errors',
        'native expected error codes',
        clipboardPayloadError.code === 'clipboard.invalid-payload' &&
          dialogPayloadError.code === 'dialog.invalid-payload' &&
          windowPayloadError.code === 'window.invalid-size' &&
          shellPayloadError.code === 'shell.invalid-payload'
          ? 'pass'
          : 'fail',
        { clipboardPayloadError, dialogPayloadError, windowPayloadError, shellPayloadError },
      );
    } catch (error) {
      pushCheck('native.expected_errors', 'native expected error codes', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      const fsDir = `notes/gui-smoke-${Date.now().toString(36)}`;
      const fsPath = `${fsDir}/hello.txt`;
      const fsCreateDir = await axion.invoke('fs.create_dir', { path: fsDir });
      const fsWrite = await axion.invoke('fs.write_text', {
        path: fsPath,
        contents: `${appName} filesystem smoke`,
      });
      const fsExists = await axion.invoke('fs.exists', { path: fsPath });
      const fsRead = await axion.invoke('fs.read_text', { path: fsPath });
      const fsList = await axion.invoke('fs.list_dir', { path: fsDir });
      const fsRemove = await axion.invoke('fs.remove', { path: fsPath });
      const fsMissing = await axion.invoke('fs.exists', { path: fsPath });
      await axion.invoke('fs.remove', { path: fsDir, recursive: true });
      fsSummary = { fsCreateDir, fsWrite, fsExists, fsRead, fsList, fsRemove, fsMissing };
      pushCheck(
        'fs.lifecycle',
        'fs create/list/remove lifecycle',
        fsExists?.exists === true &&
          fsRead?.contents === `${appName} filesystem smoke` &&
          Array.isArray(fsList?.entries) &&
          fsList.entries.some((entry) => entry.name === 'hello.txt') &&
          fsRemove?.removed === true &&
          fsMissing?.exists === false
          ? 'pass'
          : 'fail',
        fsPath,
      );
    } catch (error) {
      pushCheck('fs.lifecycle', 'fs create/list/remove lifecycle', 'fail', error instanceof Error ? error.message : String(error));
    }

    try {
      const errorDir = `notes/gui-smoke-errors-${Date.now().toString(36)}`;
      const errorFile = `${errorDir}/file.txt`;
      await axion.invoke('fs.create_dir', { path: errorDir });
      await axion.invoke('fs.write_text', { path: errorFile, contents: 'error probes' });
      const missingError = await axion.invoke('fs.read_text', { path: `${errorDir}/missing.txt` })
        .then(() => 'unexpected success')
        .catch(normalizeError);
      const listFileError = await axion.invoke('fs.list_dir', { path: errorFile })
        .then(() => 'unexpected success')
        .catch(normalizeError);
      const removeNonEmptyError = await axion.invoke('fs.remove', { path: errorDir })
        .then(() => 'unexpected success')
        .catch(normalizeError);
      await axion.invoke('fs.remove', { path: errorDir, recursive: true });
      pushCheck(
        'fs.expected_errors',
        'fs expected error codes',
        missingError.code === 'fs.not-found' &&
          listFileError.code === 'fs.not-directory' &&
          removeNonEmptyError.code === 'fs.directory-not-empty'
          ? 'pass'
          : 'fail',
        { missingError, listFileError, removeNonEmptyError },
      );
    } catch (error) {
      pushCheck('fs.expected_errors', 'fs expected error codes', 'fail', error instanceof Error ? error.message : String(error));
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
      source: '@GUI_SMOKE_SOURCE@',
      exported_at: exportedAt.toISOString(),
      exported_at_unix_seconds: Math.floor(exportedAt.getTime() / 1000),
      manifest_path: null,
      app_name: appInfo?.appName ?? appName,
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
            configured_commands: [...axion.commands],
            configured_events: [...axion.events],
            configured_protocols: [axion.protocol ?? 'axion'],
            runtime_command_count: axion.commands.length,
            runtime_event_count: axion.events.length,
            host_events: [...axion.hostEvents],
            trusted_origins: [...axion.trustedOrigins],
            allowed_navigation_origins: [],
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
      configured_clipboard_backend: clipboardRead?.backend ?? null,
      clipboard_backend: clipboardRead?.backend ?? null,
      icon: null,
      host_events: [...axion.hostEvents],
      staged_app_dir: null,
      asset_manifest_path: null,
      artifacts_removed: null,
      result,
      diagnostics: {
        bridge: bridgeInfo,
        app_version: appVersion,
        greeting,
        clipboard: clipboardRead,
        filesystem: fsSummary,
        smoke_checks: checks,
        compat_input: inputSnapshot,
      },
    };
  };

  if (runNativeApiChecks) {
    runNativeApiChecks.addEventListener('click', async () => {
      runNativeApiChecks.disabled = true;
      status.textContent = 'Running Native API checks...';
      try {
        const report = await window.__AXION_GUI_SMOKE__();
        renderJson('native-api', {
          result: report.result,
          source: report.source,
          checks: report.diagnostics?.smoke_checks ?? [],
          appVersion: report.diagnostics?.app_version ?? null,
          clipboard: report.diagnostics?.clipboard ?? null,
          filesystem: report.diagnostics?.filesystem ?? null,
          input: report.diagnostics?.compat_input ?? null,
        });
        status.textContent = `Native API checks ${report.result}`;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        renderJson('native-api', { result: 'failed', error: message });
        status.textContent = `Native API checks failed: ${message}`;
      } finally {
        runNativeApiChecks.disabled = false;
      }
    });
  }

  try {
    const pluginReady = new Promise((resolve) => {
      const timeout = window.setTimeout(() => resolve({ source: 'demo-plugin', observed: false }), 1500);
      axion.listen('demo.ready', (payload) => {
        window.clearTimeout(timeout);
        resolve({ source: payload.source, observed: true });
      });
    });

    const [ping, appInfo, appVersion, appEcho, windowInfo] = await Promise.all([
      axion.invoke('app.ping', { from: appName }),
      axion.invoke('app.info', null),
      axion.invoke('app.version', null),
      axion.invoke('app.echo', { from: appName, async: true }),
      axion.invoke('window.info', null),
    ]);
    const windowTitleUpdate = await axion.invoke('window.set_title', {
      title: `${windowInfo.title} · Axion`,
    });
    const windowSizeUpdate = await axion.invoke('window.set_size', {
      width: Math.max(windowInfo.width, 960),
      height: Math.max(windowInfo.height, 720),
    });

    renderJson('app-info', {
      bridge: typeof diagnostics?.describeBridge === 'function' ? diagnostics.describeBridge() : null,
      ping,
      appInfo,
      appVersion,
      appEcho,
      windowInfo,
      windowTitleUpdate,
      windowSizeUpdate,
      lifecycleControls: {
        appExitAvailable: axion.commands.includes('app.exit'),
        windowCloseAvailable: axion.commands.includes('window.close'),
        windowConfirmCloseAvailable: axion.commands.includes('window.confirm_close'),
        windowPreventCloseAvailable: axion.commands.includes('window.prevent_close'),
      },
    });

    const fsWrite = await axion.invoke('fs.write_text', {
      path: 'notes/hello.txt',
      contents: `${appName} wrote this through the Axion bridge`,
    });
    const fsExists = await axion.invoke('fs.exists', { path: 'notes/hello.txt' });
    const fsRead = await axion.invoke('fs.read_text', { path: 'notes/hello.txt' });
    const fsList = await axion.invoke('fs.list_dir', { path: 'notes' });
    const clipboardWrite = await axion.invoke('clipboard.write_text', {
      text: `${appName} clipboard ${new Date().toISOString()}`,
    });
    const clipboardRead = await axion.invoke('clipboard.read_text', null);
    const dialogOpen = await axion.invoke('dialog.open', {
      title: 'Select files for the Axion preview',
      multiple: true,
      filters: [
        { name: 'Text', extensions: ['txt', 'md'] },
        { name: 'Images', extensions: ['png', 'jpg'] },
      ],
    });
    const dialogSave = await axion.invoke('dialog.save', {
      title: 'Choose a save path for the Axion preview',
      defaultPath: 'notes/export.txt',
    });
    renderJson('native-api', { clipboardWrite, clipboardRead, fsWrite, fsExists, fsRead, fsList, dialogOpen, dialogSave });

    const [greeting, pluginEvent] = await Promise.all([
      axion.invoke('demo.greet', { from: `${appName}-frontend` }),
      pluginReady,
    ]);
    renderJson('custom-command', { greeting, pluginEvent });

    const deniedCommand = await axion.invoke('demo.missing', null)
      .then(() => 'unexpected success')
      .catch((error) => error instanceof Error ? error.message : String(error));
    renderJson('capability-denial', {
      attemptedCommand: 'demo.missing',
      result: deniedCommand,
      explanation: 'Commands must be registered and allowed in axion.toml before frontend code can invoke them.',
    });

    const hostLog = axion.events.includes('app.log')
      ? await axion.emit('app.log', { message: `${appName} frontend is ready`, windowId: windowInfo.id })
      : false;
    renderHostEventLog({
      emitted: hostLog,
    });

    status.textContent = `Axion bridge ready: ${ping.message} from ${ping.appName}; custom=${greeting.message}`;
  } catch (error) {
    status.textContent = `Axion invoke failed: ${error instanceof Error ? error.message : String(error)}`;
    renderText('event-log', status.textContent);
  }
});
