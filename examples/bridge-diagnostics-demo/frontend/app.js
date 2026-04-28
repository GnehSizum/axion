window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const bridgeSnapshot = document.getElementById('bridge-snapshot');
  const selfCheckResult = document.getElementById('self-check-result');
  const smokeChecklist = document.getElementById('smoke-checklist');
  const eventLog = document.getElementById('event-log');
  const savedReport = document.getElementById('saved-report');
  const compatInput = document.getElementById('compat-input');
  const compatTextarea = document.getElementById('compat-textarea');
  const compatDiagnostics = document.getElementById('compat-diagnostics');
  const actionFeedback = document.getElementById('action-feedback');

  if (
    !status ||
    !bridgeSnapshot ||
    !selfCheckResult ||
    !smokeChecklist ||
    !eventLog ||
    !savedReport ||
    !compatInput ||
    !compatTextarea ||
    !compatDiagnostics ||
    !actionFeedback
  ) {
    return;
  }

  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected.';
    bridgeSnapshot.textContent = 'Bridge unavailable';
    selfCheckResult.textContent = 'Bridge unavailable';
    compatDiagnostics.textContent = 'Bridge unavailable';
    eventLog.textContent = 'Bridge unavailable';
    savedReport.textContent = 'Bridge unavailable';
    return;
  }

  const bridge = window.__AXION__;
  const diagnostics = bridge.diagnostics;
  const compat = bridge.compat;
  const latestReportPointerPath = 'reports/latest-report.txt';
  const formatPretty = (value) =>
    typeof diagnostics?.toPrettyJson === 'function'
      ? diagnostics.toPrettyJson(value)
      : JSON.stringify(value, null, 2);
  const uniqueStrings = (values) =>
    Array.from(
      new Set(
        (Array.isArray(values) ? values : []).filter(
          (value) => typeof value === 'string' && value.length > 0,
        ),
      ),
    );
  const bridgeAllowsCommand = (command) =>
    Array.isArray(bridge.commands) && bridge.commands.includes(command);
  const describeBridge = () =>
    typeof diagnostics?.describeBridge === 'function'
      ? diagnostics.describeBridge()
      : {
          appName: bridge.appName,
          commands: bridge.commands,
          events: bridge.events,
          hostEvents: bridge.hostEvents,
          trustedOrigins: bridge.trustedOrigins,
          version: bridge.version,
          protocol: bridge.protocol,
          locationHref: window.location.href,
        };
  const snapshotControl = (element, detail) =>
    typeof diagnostics?.snapshotTextControl === 'function'
      ? diagnostics.snapshotTextControl(element, detail)
      : {
          targetId: element?.id ?? null,
          detail,
        };

  const state = {
    events: [],
    lastSelfCheck: null,
    smokeChecks: [],
    lastExport: null,
    lastLoadedReport: null,
    bridgeSnapshot: null,
    appInfo: null,
    appVersion: null,
    currentWindow: null,
    windowList: null,
    dialogPreview: null,
  };

  const renderBridgeSnapshot = () => {
    state.bridgeSnapshot = describeBridge();
    bridgeSnapshot.textContent = formatPretty(state.bridgeSnapshot);
  };

  const renderEventLog = () => {
    eventLog.textContent = state.events.length
      ? formatPretty(state.events)
      : 'Waiting for host events…';
  };

  const renderSelfCheck = () => {
    selfCheckResult.textContent = state.lastSelfCheck
      ? formatPretty(state.lastSelfCheck)
      : 'Waiting for diagnostics run…';
  };

  const renderSavedReport = () => {
    if (!state.lastLoadedReport) {
      savedReport.textContent = 'No exported report loaded yet.';
      return;
    }

    if (!state.lastLoadedReport.ok) {
      savedReport.textContent = formatPretty(state.lastLoadedReport);
      return;
    }

    savedReport.textContent = formatPretty({
      path: state.lastLoadedReport.path,
      loadedAt: state.lastLoadedReport.loadedAt,
      report: state.lastLoadedReport.report,
    });
  };

  const renderSmokeChecklist = () => {
    smokeChecklist.textContent = '';

    const checks = state.smokeChecks.length
      ? state.smokeChecks
      : [{ label: 'Waiting for smoke checklist…', status: 'pending' }];

    for (const check of checks) {
      const item = document.createElement('li');
      const label = check.detail ? `${check.label}: ${check.detail}` : check.label;
      item.textContent = label;

      if (check.status === 'pass') {
        item.classList.add('check-pass');
      } else if (check.status === 'fail') {
        item.classList.add('check-fail');
      } else if (check.status === 'skip') {
        item.classList.add('check-skip');
      }

      smokeChecklist.appendChild(item);
    }
  };

  const renderCompatDiagnostics = (element, detail = {}) => {
    compatDiagnostics.textContent = formatPretty(snapshotControl(element, detail));
  };

  const buildWindowReport = (windowState) => ({
    id: windowState?.id ?? 'main',
    title: windowState?.title ?? document.title,
    bridge_enabled: true,
    configured_commands: [...bridge.commands],
    configured_events: [...bridge.events],
    configured_protocols: [bridge.protocol ?? 'axion'],
    runtime_command_count: bridge.commands.length,
    runtime_event_count: bridge.events.length,
    host_events: uniqueStrings(bridge.hostEvents),
    trusted_origins: uniqueStrings(state.bridgeSnapshot?.trustedOrigins ?? bridge.trustedOrigins),
    allowed_navigation_origins: [],
    allow_remote_navigation: false,
    width: windowState?.width ?? null,
    height: windowState?.height ?? null,
    resizable: windowState?.resizable ?? null,
    visible: windowState?.visible ?? null,
    focused: windowState?.focused ?? null,
  });

  const buildDiagnosticsReport = () => {
    const exportedAt = new Date();
    const bridgeInfo = state.bridgeSnapshot ?? describeBridge();
    const appInfo = state.appInfo ?? {};
    const windows = Array.isArray(state.windowList?.windows) && state.windowList.windows.length
      ? state.windowList.windows.map((windowState) => buildWindowReport(windowState))
      : state.currentWindow
        ? [buildWindowReport(state.currentWindow)]
        : [];
    const dialogBackend =
      state.dialogPreview?.dialogOpen?.backend ??
      state.dialogPreview?.dialogSave?.backend ??
      null;
    const clipboardBackend =
      state.lastSelfCheck?.clipboardRead?.backend ??
      state.lastSelfCheck?.clipboardWrite?.backend ??
      null;

    const result =
      state.lastSelfCheck?.ok === false ||
      state.smokeChecks.some((check) => check.status === 'fail')
        ? 'failed'
        : 'ok';

    return {
      schema:
        diagnostics?.reportSchema ??
        bridgeInfo.diagnosticsReportSchema ??
        'axion.diagnostics-report.v1',
      source: 'bridge-diagnostics-demo',
      exported_at: exportedAt.toISOString(),
      exported_at_unix_seconds: Math.floor(exportedAt.getTime() / 1000),
      manifest_path: null,
      app_name: appInfo.appName ?? bridgeInfo.appName ?? bridge.appName,
      identifier: appInfo.identifier ?? null,
      version: appInfo.version ?? null,
      description: appInfo.description ?? null,
      authors: Array.isArray(appInfo.authors) ? appInfo.authors : [],
      homepage: appInfo.homepage ?? null,
      mode: appInfo.mode ?? null,
      window_count: windows.length,
      windows,
      frontend_dist: null,
      entry: bridgeInfo.locationHref ?? window.location.href,
      configured_dialog_backend: dialogBackend,
      dialog_backend: dialogBackend,
      configured_clipboard_backend: clipboardBackend,
      clipboard_backend: clipboardBackend,
      icon: null,
      host_events: uniqueStrings(bridgeInfo.hostEvents ?? bridge.hostEvents),
      staged_app_dir: null,
      asset_manifest_path: null,
      artifacts_removed: null,
      result,
      diagnostics: {
        bridge: bridgeInfo,
        app_version: state.appVersion,
        current_window: state.currentWindow,
        window_list: state.windowList,
        self_check: state.lastSelfCheck,
        smoke_checks: state.smokeChecks,
        recent_events: state.events,
        dialog_preview: state.dialogPreview,
        compat_input: snapshotControl(compatInput, { source: 'report-export' }),
        compat_textarea: snapshotControl(compatTextarea, { source: 'report-export' }),
        last_export: state.lastExport,
        latest_report_pointer: latestReportPointerPath,
      },
    };
  };

  const refreshRuntimeMetadata = async () => {
    const tasks = [];

    if (bridgeAllowsCommand('app.info')) {
      tasks.push(
        bridge.invoke('app.info', null).then((result) => {
          state.appInfo = result;
        }),
      );
    }

    if (bridgeAllowsCommand('app.version')) {
      tasks.push(
        bridge.invoke('app.version', null).then((result) => {
          state.appVersion = result;
        }),
      );
    }

    if (bridgeAllowsCommand('window.info')) {
      tasks.push(
        bridge.invoke('window.info', null).then((result) => {
          state.currentWindow = result;
        }),
      );
    }

    if (bridgeAllowsCommand('window.list')) {
      tasks.push(
        bridge.invoke('window.list', null).then((result) => {
          state.windowList = result;
        }),
      );
    }

    await Promise.allSettled(tasks);
    renderBridgeSnapshot();
  };

  const recordHostEvent = (name, payload) => {
    state.events.unshift({
      name,
      payload,
      receivedAt: new Date().toISOString(),
    });
    state.events = state.events.slice(0, 20);
    renderEventLog();
  };

  for (const name of bridge.hostEvents) {
    bridge.listen(name, (payload) => recordHostEvent(name, payload));
  }

  const installTextInputSelectionPatch = compat?.installTextInputSelectionPatch;
  if (typeof installTextInputSelectionPatch === 'function') {
    const patchTargets = [
      [compatInput, false],
      [compatTextarea, true],
    ];
    for (const [element, manualPointerSelection] of patchTargets) {
      installTextInputSelectionPatch(element, {
        manualPointerSelection,
        onStatus(message) {
          status.textContent = message;
        },
        onUpdate(snapshot) {
          compatDiagnostics.textContent = formatPretty(snapshot);
        },
      });
    }
  } else {
    compatDiagnostics.textContent =
      'window.__AXION__.compat.installTextInputSelectionPatch is unavailable';
  }

  compatInput.addEventListener('focus', () =>
    renderCompatDiagnostics(compatInput, { source: 'focus' }),
  );
  compatTextarea.addEventListener('focus', () =>
    renderCompatDiagnostics(compatTextarea, { source: 'focus' }),
  );
  compatTextarea.addEventListener('keydown', (event) => {
    if (event.key !== 'Tab') return;
    event.preventDefault();
    const start = compatTextarea.selectionStart ?? compatTextarea.value.length;
    const end = compatTextarea.selectionEnd ?? start;
    const value = compatTextarea.value;
    compatTextarea.value = `${value.slice(0, start)}\t${value.slice(end)}`;
    compatTextarea.selectionStart = start + 1;
    compatTextarea.selectionEnd = start + 1;
    renderCompatDiagnostics(compatTextarea, { source: 'textarea-tab-handler' });
  });

  const loadLatestReport = async ({ silent = false } = {}) => {
    if (!bridgeAllowsCommand('fs.read_text')) {
      state.lastLoadedReport = {
        ok: false,
        error: '`fs.read_text` is not allowed in this window.',
        loadedAt: new Date().toISOString(),
      };
      renderSavedReport();
      return;
    }

    if (!silent) {
      actionFeedback.textContent = 'Reading latest diagnostics report...';
    }

    try {
      const latestPointer = await bridge.invoke('fs.read_text', {
        path: latestReportPointerPath,
      });
      const reportPath = latestPointer?.contents?.trim();
      if (!reportPath) {
        throw new Error('Latest report pointer is empty.');
      }

      const reportFile = await bridge.invoke('fs.read_text', {
        path: reportPath,
      });
      const report = JSON.parse(reportFile.contents);
      state.lastLoadedReport = {
        ok: true,
        path: reportPath,
        loadedAt: new Date().toISOString(),
        report,
      };
      renderSavedReport();
      if (!silent) {
        actionFeedback.textContent = `Loaded diagnostics report from ${reportPath}.`;
      }
    } catch (error) {
      if (silent) {
        renderSavedReport();
        return;
      }

      const message = error instanceof Error ? error.message : String(error);
      state.lastLoadedReport = {
        ok: false,
        error: message,
        loadedAt: new Date().toISOString(),
      };
      renderSavedReport();
      actionFeedback.textContent = `Unable to read latest report: ${message}`;
    }
  };

  const runSelfCheck = async () => {
    status.textContent = 'Running diagnostics self-check…';
    actionFeedback.textContent = 'Running bridge checks...';

    try {
      const ping = await bridge.invoke('app.ping', { from: 'bridge-diagnostics-demo' });
      const [appInfo, appVersion, appEcho, windowInfo, windowList] = await Promise.all([
        bridge.invoke('app.info', null),
        bridge.invoke('app.version', null),
        bridge.invoke('app.echo', { from: 'bridge-diagnostics-demo', async: true }),
        bridge.invoke('window.info', null),
        bridgeAllowsCommand('window.list')
          ? bridge.invoke('window.list', null)
          : Promise.resolve(state.windowList),
      ]);
      const fsWrite = await bridge.invoke('fs.write_text', {
        path: 'notes/diagnostics.txt',
        contents: `bridge-diagnostics-demo wrote this at ${new Date().toISOString()}`,
      });
      const fsRead = await bridge.invoke('fs.read_text', {
        path: 'notes/diagnostics.txt',
      });
      const clipboardWrite = await bridge.invoke('clipboard.write_text', {
        text: `bridge-diagnostics-demo clipboard ${new Date().toISOString()}`,
      });
      const clipboardRead = await bridge.invoke('clipboard.read_text', null);

      state.appInfo = appInfo;
      state.appVersion = appVersion;
      state.currentWindow = windowInfo;
      state.windowList = windowList;
      renderBridgeSnapshot();

      state.lastSelfCheck = {
        ok: true,
        ranAt: new Date().toISOString(),
        bridge: state.bridgeSnapshot,
        ping,
        appInfo,
        appVersion,
        appEcho,
        windowInfo,
        windowList,
        fsWrite,
        fsRead,
        clipboardWrite,
        clipboardRead,
      };
      status.textContent = `Diagnostics ready: ${ping.message} from ${ping.appName}`;
      actionFeedback.textContent = 'Diagnostics self-check passed.';
      renderSelfCheck();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      state.lastSelfCheck = {
        ok: false,
        ranAt: new Date().toISOString(),
        error: message,
      };
      status.textContent = `Diagnostics self-check failed: ${message}`;
      actionFeedback.textContent = 'Diagnostics self-check failed.';
      renderSelfCheck();
    }
  };

  const runSmokeChecklist = async () => {
    const checks = [];
    const pushCheck = (id, label, statusValue, detail) => {
      checks.push({ id, label, status: statusValue, detail });
    };
    const expectFailure = new URLSearchParams(window.location.search).has(
      'axion_smoke_expect_fail',
    );

    const bridgeInfo =
      typeof diagnostics?.describeBridge === 'function'
        ? diagnostics.describeBridge()
        : null;

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
      typeof compat?.installTextInputSelectionPatch === 'function' ? 'pass' : 'fail',
      typeof compat?.installTextInputSelectionPatch === 'function'
        ? 'installTextInputSelectionPatch'
        : 'compat helper missing',
    );

    if (bridgeInfo) {
      pushCheck(
        'security.current_origin',
        'Current origin trusted',
        bridgeInfo.trustedOrigins.includes(bridgeInfo.currentOrigin) ? 'pass' : 'fail',
        bridgeInfo.currentOrigin,
      );
    }

    if (bridgeAllowsCommand('window.list')) {
      try {
        const windowList = await bridge.invoke('window.list', null);
        state.windowList = windowList;
        pushCheck(
          'window.list',
          'window.list',
          Array.isArray(windowList?.windows) ? 'pass' : 'fail',
          `${windowList?.windows?.length ?? 0} window(s)`,
        );
      } catch (error) {
        pushCheck(
          'window.list',
          'window.list',
          'fail',
          error instanceof Error ? error.message : String(error),
        );
      }
    }

    try {
      const ping = await bridge.invoke('app.ping', { from: 'bridge-diagnostics-smoke' });
      pushCheck('app.ping', 'app.ping', 'pass', ping.message);
    } catch (error) {
      pushCheck(
        'app.ping',
        'app.ping',
        'fail',
        error instanceof Error ? error.message : String(error),
      );
    }

    try {
      const echo = await bridge.invoke('app.echo', { from: 'bridge-diagnostics-smoke', async: true });
      pushCheck('app.echo', 'app.echo', 'pass', echo?.payload?.from ?? 'echo returned');
    } catch (error) {
      pushCheck(
        'app.echo',
        'app.echo',
        'fail',
        error instanceof Error ? error.message : String(error),
      );
    }

    try {
      await bridge.invoke('clipboard.write_text', {
        text: 'bridge-diagnostics clipboard smoke',
      });
      const clipboardRead = await bridge.invoke('clipboard.read_text', null);
      pushCheck(
        'clipboard.roundtrip',
        'clipboard roundtrip',
        clipboardRead?.text === 'bridge-diagnostics clipboard smoke' ? 'pass' : 'fail',
        clipboardRead?.backend ?? 'missing backend',
      );
    } catch (error) {
      pushCheck(
        'clipboard.roundtrip',
        'clipboard roundtrip',
        'fail',
        error instanceof Error ? error.message : String(error),
      );
    }

    try {
      const fsPath = `notes/smoke-${Date.now().toString(36)}.txt`;
      await bridge.invoke('fs.write_text', {
        path: fsPath,
        contents: `smoke:${new Date().toISOString()}`,
      });
      const fsRead = await bridge.invoke('fs.read_text', { path: fsPath });
      pushCheck(
        'fs.roundtrip',
        'fs roundtrip',
        typeof fsRead?.contents === 'string' ? 'pass' : 'fail',
        fsPath,
      );
    } catch (error) {
      pushCheck(
        'fs.roundtrip',
        'fs roundtrip',
        'fail',
        error instanceof Error ? error.message : String(error),
      );
    }

    try {
      const [dialogOpen, dialogSave] = await Promise.all([
        bridge.invoke('dialog.open', {
          title: 'Smoke checklist open preview',
          multiple: true,
          filters: [{ name: 'Text', extensions: ['txt', 'md'] }],
        }),
        bridge.invoke('dialog.save', {
          title: 'Smoke checklist save preview',
          defaultPath: 'notes/smoke-export.txt',
        }),
      ]);
      state.dialogPreview = {
        ranAt: new Date().toISOString(),
        dialogOpen,
        dialogSave,
      };
      pushCheck(
        'dialog.preview',
        'dialog preview',
        'pass',
        `open=${dialogOpen.backend}, save=${dialogSave.backend}`,
      );
    } catch (error) {
      pushCheck(
        'dialog.preview',
        'dialog preview',
        'fail',
        error instanceof Error ? error.message : String(error),
      );
    }

    if (bridge.events.includes('app.log')) {
      try {
        await bridge.emit('app.log', {
          message: 'bridge-diagnostics smoke checklist emitted app.log',
          source: 'bridge-diagnostics-demo',
        });
        pushCheck('event.app_log_emit', 'app.log emit', 'pass', 'event emitted');
      } catch (error) {
        pushCheck(
          'event.app_log_emit',
          'app.log emit',
          'fail',
          error instanceof Error ? error.message : String(error),
        );
      }
    } else {
      pushCheck(
        'event.app_log_emit',
        'app.log emit',
        'skip',
        'event not allowed in this window',
      );
    }

    const inputSnapshot = snapshotControl(compatInput, {
      source: 'smoke-checklist',
    });
    pushCheck(
      'input.snapshot',
      'Text control snapshot',
      inputSnapshot && inputSnapshot.targetId === compatInput.id ? 'pass' : 'fail',
      inputSnapshot ? inputSnapshot.targetId : 'snapshotTextControl unavailable',
    );
    if (expectFailure) {
      pushCheck(
        'diagnostics.expected_failure',
        'Expected failure injection',
        'fail',
        'axion_smoke_expect_fail query parameter is set',
      );
    }

    state.smokeChecks = checks;
    renderSmokeChecklist();
  };

  const emitHostLog = async () => {
    actionFeedback.textContent = 'Emitting host log...';
    if (!bridge.events.includes('app.log')) {
      actionFeedback.textContent = '`app.log` is not allowed in this window.';
      return;
    }

    await bridge.emit('app.log', {
      message: 'bridge-diagnostics-demo emitted app.log',
      source: 'bridge-diagnostics-demo',
    });
    actionFeedback.textContent = 'Emitted `app.log` successfully.';
  };

  const runDialogPreview = async () => {
    actionFeedback.textContent = 'Running dialog preview...';
    const [dialogOpen, dialogSave] = await Promise.all([
      bridge.invoke('dialog.open', {
        title: 'Diagnostics open preview',
        multiple: true,
        filters: [{ name: 'Text', extensions: ['txt', 'md'] }],
      }),
      bridge.invoke('dialog.save', {
        title: 'Diagnostics save preview',
        defaultPath: 'notes/export.txt',
      }),
    ]);
    state.dialogPreview = {
      ranAt: new Date().toISOString(),
      dialogOpen,
      dialogSave,
    };
    state.lastSelfCheck = {
      ok: true,
      ranAt: new Date().toISOString(),
      dialogOpen,
      dialogSave,
    };
    actionFeedback.textContent = 'Dialog preview completed.';
    renderSelfCheck();
  };

  const exportReport = async () => {
    actionFeedback.textContent = 'Exporting diagnostics report...';
    await refreshRuntimeMetadata();
    const timestamp = new Date().toISOString().replaceAll(':', '-');
    const path = `reports/bridge-diagnostics-${timestamp}.json`;
    const report = buildDiagnosticsReport();

    try {
      const result = await bridge.invoke('fs.write_text', {
        path,
        contents: formatPretty(report),
      });
      await bridge.invoke('fs.write_text', {
        path: latestReportPointerPath,
        contents: result.path ?? path,
      });
      state.lastExport = {
        ok: true,
        path: result.path ?? path,
        pointerPath: latestReportPointerPath,
        exportedAt: new Date().toISOString(),
      };
      state.lastLoadedReport = {
        ok: true,
        path: state.lastExport.path,
        loadedAt: new Date().toISOString(),
        report,
      };
      actionFeedback.textContent = `Exported diagnostics report to ${state.lastExport.path}.`;
      state.lastSelfCheck = {
        ...(state.lastSelfCheck ?? {}),
        reportExport: state.lastExport,
      };
      renderSelfCheck();
      renderSavedReport();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      state.lastExport = {
        ok: false,
        path,
        exportedAt: new Date().toISOString(),
        error: message,
      };
      actionFeedback.textContent = `Export failed: ${message}`;
      renderSelfCheck();
    }
  };

  window.__AXION_GUI_SMOKE__ = async () => {
    await runSelfCheck();
    await runSmokeChecklist();
    await runDialogPreview();
    await refreshRuntimeMetadata();
    const report = buildDiagnosticsReport();
    state.lastLoadedReport = {
      ok: report.result === 'ok',
      path: 'AXION_GUI_SMOKE',
      loadedAt: new Date().toISOString(),
      report,
    };
    renderSelfCheck();
    renderSmokeChecklist();
    renderSavedReport();
    return report;
  };

  document.getElementById('run-self-check')?.addEventListener('click', () => {
    void runSelfCheck();
  });
  document.getElementById('emit-host-log')?.addEventListener('click', () => {
    void emitHostLog();
  });
  document.getElementById('run-dialog-preview')?.addEventListener('click', () => {
    void runDialogPreview();
  });
  document.getElementById('export-report')?.addEventListener('click', () => {
    void exportReport();
  });
  document.getElementById('read-latest-report')?.addEventListener('click', () => {
    void loadLatestReport();
  });

  await refreshRuntimeMetadata();
  renderBridgeSnapshot();
  renderEventLog();
  renderSelfCheck();
  renderSmokeChecklist();
  renderSavedReport();
  renderCompatDiagnostics(compatInput, { source: 'initial' });
  void loadLatestReport({ silent: true });
  void runSelfCheck();
  void runSmokeChecklist();
});
