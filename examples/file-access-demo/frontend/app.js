window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
  const operationResult = document.getElementById('operation-result');
  const eventLog = document.getElementById('event-log');
  const summaryApp = document.getElementById('summary-app');
  const summaryWindow = document.getElementById('summary-window');
  const summaryCommands = document.getElementById('summary-commands');
  const summaryEvents = document.getElementById('summary-events');
  const filePath = document.getElementById('file-path');
  const fileContents = document.getElementById('file-contents');
  const dialogDefaultPath = document.getElementById('dialog-default-path');
  const fileFeedback = document.getElementById('file-feedback');
  const dialogFeedback = document.getElementById('dialog-feedback');
  const inputDiagnostics = document.getElementById('input-diagnostics');

  if (
    !status ||
    !details ||
    !operationResult ||
    !eventLog ||
    !summaryApp ||
    !summaryWindow ||
    !summaryCommands ||
    !summaryEvents ||
    !filePath ||
    !fileContents ||
    !dialogDefaultPath ||
    !fileFeedback ||
    !dialogFeedback ||
    !inputDiagnostics
  ) {
    return;
  }

  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected.';
    details.textContent = 'Bridge unavailable';
    return;
  }

  const bridge = window.__AXION__;
  const diagnostics = bridge.diagnostics;
  const installTextInputSelectionPatch = bridge.compat?.installTextInputSelectionPatch;
  const state = {
    appName: bridge.appName,
    commands: [...bridge.commands],
    events: [...bridge.events],
    hostEvents: [...bridge.hostEvents],
    trustedOrigins: [...bridge.trustedOrigins],
    appInfo: null,
    windowInfo: null,
    ping: null,
    deniedProbes: {},
    eventLog: [],
    lastOperation: null,
  };

  const formatJson = (value) =>
    typeof diagnostics?.toPrettyJson === 'function'
      ? diagnostics.toPrettyJson(value)
      : JSON.stringify(value, null, 2);

  const render = () => {
    summaryApp.textContent = state.appInfo
      ? `${state.appInfo.appName} (${state.appInfo.mode})`
      : 'Loading…';
    summaryWindow.textContent = state.windowInfo
      ? `${state.windowInfo.id} · ${state.windowInfo.title} · ${state.windowInfo.width}x${state.windowInfo.height}`
      : 'Loading…';
    summaryCommands.textContent = state.commands.join(', ');
    summaryEvents.textContent = [...state.events, ...state.hostEvents].join(', ');
    details.textContent = formatJson({
      bridge: typeof diagnostics?.describeBridge === 'function' ? diagnostics.describeBridge() : null,
      ping: state.ping,
      appInfo: state.appInfo,
      windowInfo: state.windowInfo,
      trustedOrigins: state.trustedOrigins,
      deniedProbes: state.deniedProbes,
    });
    operationResult.textContent = state.lastOperation
      ? formatJson(state.lastOperation)
      : 'Waiting for interaction…';
    eventLog.textContent = state.eventLog.length
      ? formatJson(state.eventLog)
      : 'Waiting for host events…';
  };

  const setLastOperation = (name, ok, payload) => {
    state.lastOperation = { name, ok, payload };
    render();
  };

  const setStatus = (message) => {
    status.textContent = message;
  };

  const setFeedback = (element, message) => {
    element.textContent = message;
  };

  const updateInputDiagnostics = (element, detail = {}) => {
    const snapshot =
      typeof diagnostics?.snapshotTextControl === 'function'
        ? diagnostics.snapshotTextControl(element, detail)
        : {
            targetId: element?.id ?? null,
            selectionStart:
              typeof element?.selectionStart === 'number' ? element.selectionStart : null,
            selectionEnd: typeof element?.selectionEnd === 'number' ? element.selectionEnd : null,
            valueLength: typeof element?.value === 'string' ? element.value.length : null,
            detail,
          };
    inputDiagnostics.textContent = formatJson(snapshot);
  };

  const markInputEvent = (label) => {
    setStatus(`Input received: ${label}`);
  };

  const setBusy = (button, busy) => {
    if (!button || typeof button !== 'object' || !('disabled' in button)) return;
    button.disabled = busy;
  };

  const invokeAndTrack = async (name, payload, button) => {
    try {
      setBusy(button, true);
      setStatus(`Running ${name}…`);
      const result = await bridge.invoke(name, payload);
      setLastOperation(name, true, result);
      setStatus(`${name} succeeded.`);
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastOperation(name, false, { error: message, payload });
      setStatus(`${name} failed: ${message}`);
      throw error;
    } finally {
      setBusy(button, false);
    }
  };

  const emitAndTrack = async (name, payload, button) => {
    try {
      setBusy(button, true);
      setStatus(`Running ${name}…`);
      const result = await bridge.emit(name, payload);
      setLastOperation(name, true, { emitted: result, payload });
      setStatus(`${name} emitted successfully.`);
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLastOperation(name, false, { error: message, payload });
      setStatus(`${name} failed: ${message}`);
      throw error;
    } finally {
      setBusy(button, false);
    }
  };

  const registerAction = (id, handler) => {
    const element = document.getElementById(id);
    if (!element) return;

    let inFlight = false;
    const run = async (label, event) => {
      if (event) {
        event.preventDefault?.();
        event.stopPropagation?.();
      }
      if (inFlight) return;
      inFlight = true;
      markInputEvent(`${id}:${label}`);
      try {
        await handler(event);
      } finally {
        inFlight = false;
      }
    };

    element.onclick = (event) => {
      void run('onclick', event);
    };

    for (const eventName of ['click', 'mouseup', 'pointerup', 'touchend']) {
      element.addEventListener(eventName, (event) => {
        void run(eventName, event);
      });
    }

    element.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        void run(`keydown:${event.key}`, event);
      }
    });
  };

  for (const name of bridge.hostEvents) {
    bridge.listen(name, (payload) => {
      state.eventLog.unshift({
        name,
        payload,
        receivedAt: new Date().toISOString(),
      });
      state.eventLog = state.eventLog.slice(0, 12);
      if (name === 'app.ready') {
        state.ready = payload;
      }
      render();
    });
  }

  fileContents.addEventListener('keydown', (event) => {
    if (event.key !== 'Tab') return;
    event.preventDefault();

    const start = fileContents.selectionStart ?? fileContents.value.length;
    const end = fileContents.selectionEnd ?? start;
    const value = fileContents.value;
    fileContents.value = `${value.slice(0, start)}\t${value.slice(end)}`;
    fileContents.selectionStart = start + 1;
    fileContents.selectionEnd = start + 1;
    setFeedback(fileFeedback, 'Inserted a tab character into the textarea.');
    setStatus('Textarea handled Tab locally.');
    updateInputDiagnostics(fileContents, { source: 'textarea-tab-handler' });
  });

  for (const element of [filePath, fileContents, dialogDefaultPath]) {
    const updateFromEvent = (source, event) => {
      const detail = {
        source,
        clientX: 'clientX' in event ? event.clientX : null,
        clientY: 'clientY' in event ? event.clientY : null,
        offsetX: 'offsetX' in event ? event.offsetX : null,
        offsetY: 'offsetY' in event ? event.offsetY : null,
      };

      window.setTimeout(() => updateInputDiagnostics(element, detail), 0);
    };

    element.addEventListener('focus', (event) => updateFromEvent('focus', event));
    element.addEventListener('click', (event) => updateFromEvent('click', event));
    element.addEventListener('pointerup', (event) => updateFromEvent('pointerup', event));
    element.addEventListener('keyup', (event) =>
      updateInputDiagnostics(element, { source: `keyup:${event.key}` }),
    );
    element.addEventListener('input', () =>
      updateInputDiagnostics(element, { source: 'input' }),
    );
    element.addEventListener('select', () =>
      updateInputDiagnostics(element, { source: 'select' }),
    );

    if (typeof installTextInputSelectionPatch === 'function') {
      installTextInputSelectionPatch(element, {
        manualPointerSelection: element === fileContents,
        onUpdate: (payload) => updateInputDiagnostics(element, payload.detail ?? payload),
        onStatus: setStatus,
      });
    }
  }

  registerAction('write-text', async (event) => {
    const result = await invokeAndTrack(
      'fs.write_text',
      {
        path: filePath.value,
        contents: fileContents.value,
      },
      event.currentTarget,
    );
    setFeedback(fileFeedback, `Wrote ${result.path}.`);
  });

  registerAction('read-text', async (event) => {
    const result = await invokeAndTrack(
      'fs.read_text',
      {
        path: filePath.value,
      },
      event.currentTarget,
    );
    if (typeof result?.contents === 'string') {
      fileContents.value = result.contents;
    }
    setFeedback(fileFeedback, `Read ${result.path}.`);
  });

  registerAction('probe-invalid-path', async (event) => {
    try {
      await invokeAndTrack(
        'fs.read_text',
        {
          path: '../secrets.txt',
        },
        event.currentTarget,
      );
    } catch (_error) {
      setStatus('Invalid-path probe was rejected as expected.');
      setFeedback(fileFeedback, 'Rejected `../secrets.txt` as expected.');
    }
  });

  registerAction('dialog-open', async (event) => {
    const result = await invokeAndTrack(
      'dialog.open',
      {
        title: 'Preview open dialog',
        multiple: true,
        filters: [
          { name: 'Text', extensions: ['txt', 'md'] },
          { name: 'Images', extensions: ['png', 'jpg'] },
        ],
      },
      event.currentTarget,
    );
    setFeedback(dialogFeedback, `dialog.open returned backend=${result.backend}, canceled=${result.canceled}.`);
  });

  registerAction('dialog-save', async (event) => {
    const result = await invokeAndTrack(
      'dialog.save',
      {
        title: 'Preview save dialog',
        defaultPath: dialogDefaultPath.value,
      },
      event.currentTarget,
    );
    setFeedback(dialogFeedback, `dialog.save returned backend=${result.backend}, canceled=${result.canceled}.`);
  });

  registerAction('emit-log', async (event) => {
    if (!bridge.events.includes('app.log')) {
      setLastOperation('app.log', false, { error: 'app.log is not allowed in this window' });
      setFeedback(dialogFeedback, '`app.log` is not allowed in this window.');
      return;
    }
    await emitAndTrack(
      'app.log',
      {
        message: 'file-access-demo frontend emitted app.log',
        windowId: state.windowInfo?.id ?? null,
        path: filePath.value,
      },
      event.currentTarget,
    );
    setFeedback(dialogFeedback, 'Emitted `app.log` successfully.');
  });

  try {
    state.ping = await bridge.invoke('app.ping', { from: 'file-access-demo' });
    state.appInfo = await bridge.invoke('app.info', null);
    state.windowInfo = await bridge.invoke('window.info', null);

    const initialWrite = await bridge.invoke('fs.write_text', {
      path: filePath.value,
      contents: fileContents.value,
    });
    const initialRead = await bridge.invoke('fs.read_text', {
      path: filePath.value,
    });
    setLastOperation('bootstrap.fs', true, { initialWrite, initialRead });

    for (const command of ['window.set_title', 'window.set_size', 'demo.greet']) {
      if (bridge.commands.includes(command)) continue;
      try {
        await bridge.invoke(command, null);
        state.deniedProbes[command] = 'unexpectedly allowed';
      } catch (error) {
        state.deniedProbes[command] = error instanceof Error ? error.message : String(error);
      }
    }

    setStatus(`Axion file bridge ready: ${state.ping.message} · ${filePath.value}`);
    setFeedback(fileFeedback, `Bootstrap wrote and read ${filePath.value}.`);
    setFeedback(dialogFeedback, 'Ready for dialog preview actions.');
    render();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setStatus(`Axion bootstrap failed: ${message}`);
    setLastOperation('bootstrap', false, { error: message });
  }
});
