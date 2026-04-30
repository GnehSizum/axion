window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
  const closeSettingsButton = document.getElementById('close-settings');
  const closePreviewButton = document.getElementById('close-preview');
  const appExitButton = document.getElementById('app-exit');
  const preventCloseCheckbox = document.getElementById('prevent-close');
  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected.';
    details.textContent = 'Bridge unavailable';
    return;
  }

  const bridge = window.__AXION__;
  const diagnostics = bridge.diagnostics;
  const formatPretty = (value) =>
    typeof diagnostics?.toPrettyJson === 'function'
      ? diagnostics.toPrettyJson(value)
      : JSON.stringify(value, null, 2);
  const results = {
    appName: bridge.appName,
    commands: [...bridge.commands],
    frontendEvents: [...bridge.events],
    hostEvents: [...bridge.hostEvents],
    trustedOrigins: [...bridge.trustedOrigins],
    allowedCalls: {},
    deniedProbes: {},
    lifecycleEvents: [],
    manualCloseDecision: false,
  };

  const waitFor = async (predicate, timeoutMs = 1200) => {
    const startedAt = Date.now();
    while (Date.now() - startedAt < timeoutMs) {
      const value = predicate();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`timed out after ${timeoutMs}ms`);
  };
  const waitForLifecycleEvent = (name, predicate, timeoutMs = 4200) =>
    waitFor(
      () =>
        results.lifecycleEvents.find(
          (event) => event.name === name && (!predicate || predicate(event.payload)),
        ),
      timeoutMs,
    );

  const closeTargetWithDecision = async (target, decision) => {
    const closeRequest = await bridge.invoke('window.close', { target });
    const requestId = closeRequest?.requestId;
    if (typeof requestId !== 'string') {
      throw new Error(`window.close did not return requestId for ${target}`);
    }
    if (decision === 'confirm') {
      await bridge.invoke('window.confirm_close', { requestId });
    } else if (decision === 'prevent') {
      await bridge.invoke('window.prevent_close', { requestId });
    }
    return closeRequest;
  };

  window.__AXION_GUI_SMOKE__ = async () => {
    const checks = [];
    const addCheck = (id, status, detail) => checks.push({ id, status, detail });
    await new Promise((resolve) => setTimeout(resolve, 100));

    try {
      if (bridge.commands.includes('app.ping')) {
        const ping = await bridge.invoke('app.ping', { from: 'multi-window-gui-smoke' });
        addCheck('app.ping', 'pass', formatPretty(ping));
      } else {
        addCheck('app.ping', 'skip', 'not allowed for this window');
      }

      if (bridge.commands.includes('window.info')) {
        const info = await bridge.invoke('window.info', null);
        addCheck('window.info', 'pass', formatPretty(info));
      } else {
        addCheck('window.info', 'skip', 'not allowed for this window');
      }

      const closeCommandsAvailable =
        bridge.commands.includes('window.close') &&
        bridge.commands.includes('window.prevent_close') &&
        bridge.hostEvents.includes('window.close_requested');
      if (closeCommandsAvailable) {
        if (preventCloseCheckbox) preventCloseCheckbox.checked = true;
        results.manualCloseDecision = true;
        const closeRequest = await bridge.invoke('window.close', null);
        addCheck(
          'window.close.pending',
          closeRequest?.pending === true && typeof closeRequest?.requestId === 'string'
            ? 'pass'
            : 'fail',
          formatPretty(closeRequest),
        );

        const closeEvent = await waitFor(() =>
          results.lifecycleEvents.find(
            (event) =>
              event.name === 'window.close_requested' &&
              event.payload?.requestId === closeRequest?.requestId,
          ),
        );
        addCheck(
          'window.close_requested.event',
          closeEvent?.payload?.timeoutMs === 3000 ? 'pass' : 'fail',
          formatPretty(closeEvent?.payload),
        );

        const prevented = await bridge.invoke('window.prevent_close', {
          requestId: closeRequest.requestId,
        });
        results.allowedCalls.lastCloseDecision = prevented;
        addCheck(
          'window.prevent_close',
          prevented?.prevented === true ? 'pass' : 'fail',
          formatPretty(prevented),
        );

        const preventedEvent = await waitFor(() =>
          results.lifecycleEvents.find(
            (event) =>
              event.name === 'window.close_prevented' &&
              event.payload?.requestId === closeRequest?.requestId,
          ),
        );
        addCheck(
          'window.close_prevented.event',
          preventedEvent?.payload?.status === 'prevented' ? 'pass' : 'fail',
          formatPretty(preventedEvent?.payload),
        );
        results.manualCloseDecision = false;

        try {
          await bridge.invoke('window.prevent_close', {
            requestId: closeRequest.requestId,
          });
          addCheck('window.prevent_close.duplicate', 'fail', 'duplicate request was accepted');
        } catch (error) {
          addCheck(
            'window.prevent_close.duplicate',
            'pass',
            error instanceof Error ? error.message : String(error),
          );
        }
      } else {
        addCheck('window.close.lifecycle', 'skip', 'close lifecycle commands not available');
      }

      const targetedCloseAvailable =
        bridge.commands.includes('window.list') &&
        bridge.commands.includes('window.close') &&
        bridge.commands.includes('window.confirm_close') &&
        bridge.hostEvents.includes('window.close_completed') &&
        bridge.hostEvents.includes('window.close_timed_out') &&
        bridge.hostEvents.includes('window.closed');
      if (targetedCloseAvailable) {
        const settingsClose = await closeTargetWithDecision('settings', 'confirm');
        addCheck(
          'window.close.settings.pending',
          settingsClose?.pending === true && settingsClose?.window?.id === 'settings'
            ? 'pass'
            : 'fail',
          formatPretty(settingsClose),
        );

        const completedEvent = await waitForLifecycleEvent(
          'window.close_completed',
          (payload) =>
            payload?.windowId === 'settings' &&
            payload?.requestId === settingsClose.requestId &&
            payload?.status === 'completed',
        );
        addCheck(
          'window.close_completed.event',
          completedEvent?.payload?.status === 'completed' ? 'pass' : 'fail',
          formatPretty(completedEvent?.payload),
        );

        const settingsClosedEvent = await waitForLifecycleEvent(
          'window.closed',
          (payload) => payload?.windowId === 'settings',
        );
        addCheck(
          'window.closed.settings.event',
          settingsClosedEvent?.payload?.windowId === 'settings' ? 'pass' : 'fail',
          formatPretty(settingsClosedEvent?.payload),
        );

        const previewClose = await closeTargetWithDecision('preview', 'timeout');
        addCheck(
          'window.close.preview.pending',
          previewClose?.pending === true && previewClose?.window?.id === 'preview'
            ? 'pass'
            : 'fail',
          formatPretty(previewClose),
        );

        const timedOutEvent = await waitForLifecycleEvent(
          'window.close_timed_out',
          (payload) =>
            payload?.windowId === 'preview' &&
            payload?.requestId === previewClose.requestId &&
            payload?.status === 'timed_out',
        );
        addCheck(
          'window.close_timed_out.event',
          timedOutEvent?.payload?.status === 'timed_out' ? 'pass' : 'fail',
          formatPretty(timedOutEvent?.payload),
        );

        const previewClosedEvent = await waitForLifecycleEvent(
          'window.closed',
          (payload) => payload?.windowId === 'preview',
        );
        addCheck(
          'window.closed.preview.event',
          previewClosedEvent?.payload?.windowId === 'preview' ? 'pass' : 'fail',
          formatPretty(previewClosedEvent?.payload),
        );
      } else {
        addCheck(
          'window.close.targeted_lifecycle',
          'skip',
          'targeted close lifecycle is not fully available',
        );
      }

      const appExitLifecycleAvailable =
        bridge.commands.includes('app.exit') &&
        bridge.commands.includes('window.prevent_close') &&
        bridge.hostEvents.includes('app.exit_requested') &&
        bridge.hostEvents.includes('app.exit_prevented') &&
        bridge.hostEvents.includes('window.close_prevented');
      if (appExitLifecycleAvailable) {
        if (preventCloseCheckbox) preventCloseCheckbox.checked = true;
        results.manualCloseDecision = true;
        try {
          const exitRequest = await bridge.invoke('app.exit', null);
          results.allowedCalls.appExitSmoke = exitRequest;
          addCheck(
            'app.exit.pending',
            exitRequest?.pending === true &&
              typeof exitRequest?.requestId === 'string' &&
              Number.isInteger(exitRequest?.windowCount)
              ? 'pass'
              : 'fail',
            formatPretty(exitRequest),
          );

          const exitEvent = await waitFor(() =>
            results.lifecycleEvents.find(
              (event) =>
                event.name === 'app.exit_requested' &&
                event.payload?.requestId === exitRequest?.requestId,
            ),
          );
          addCheck(
            'app.exit_requested.event',
            exitEvent?.payload?.defaultAction === 'request-window-close' ? 'pass' : 'fail',
            formatPretty(exitEvent?.payload),
          );

          const appExitCloseEvent = await waitFor(() =>
            results.lifecycleEvents.find(
              (event) =>
                event.name === 'window.close_requested' &&
                event.payload?.reason === 'app-exit',
            ),
          );
          const preventedExitClose = await bridge.invoke('window.prevent_close', {
            requestId: appExitCloseEvent.payload.requestId,
          });
          results.allowedCalls.lastAppExitCloseDecision = preventedExitClose;
          addCheck(
            'app.exit.window.prevent_close',
            preventedExitClose?.prevented === true ? 'pass' : 'fail',
            formatPretty(preventedExitClose),
          );

          const preventedExitEvent = await waitFor(() =>
            results.lifecycleEvents.find(
              (event) =>
                event.name === 'app.exit_prevented' &&
                event.payload?.requestId === exitRequest?.requestId,
            ),
          );
          addCheck(
            'app.exit_prevented.event',
            preventedExitEvent?.payload?.preventedCount === 1 &&
              preventedExitEvent?.payload?.closeRequests?.some(
                (entry) => entry.requestId === appExitCloseEvent.payload.requestId,
              ) &&
              preventedExitEvent?.payload?.preventedRequests?.some(
                (entry) => entry.requestId === appExitCloseEvent.payload.requestId,
              )
              ? 'pass'
              : 'fail',
            formatPretty(preventedExitEvent?.payload),
          );
        } finally {
          results.manualCloseDecision = false;
        }
      } else {
        addCheck('app.exit.lifecycle', 'skip', 'app exit lifecycle is not fully available');
      }

    } catch (error) {
      addCheck('bridge.invoke', 'fail', error instanceof Error ? error.message : String(error));
    }

    return {
      schema: 'axion.diagnostics-report.v1',
      source: 'multi-window gui smoke',
      result: checks.some((check) => check.status === 'fail') ? 'failed' : 'ok',
      app_name: bridge.appName,
      diagnostics: {
        smoke_checks: checks,
        lifecycle_events: results.lifecycleEvents,
        bridge: typeof diagnostics?.describeBridge === 'function' ? diagnostics.describeBridge() : null,
      },
    };
  };

  const render = () => {
    const windowLabel =
      results.ready?.windowId ||
      results.windowInfo?.id ||
      (bridge.commands.includes('window.info') ? 'settings-like window' : 'main-like window');
    status.textContent = `Axion multi-window bridge ready: ${windowLabel}`;
    details.textContent = formatPretty({
      bridge: typeof diagnostics?.describeBridge === 'function' ? diagnostics.describeBridge() : null,
      ...results,
    });
  };

  for (const name of bridge.hostEvents) {
    bridge.listen(name, async (payload) => {
      if (name === 'app.ready') {
        results.ready = payload;
      } else if (name.startsWith('window.') || name.startsWith('app.')) {
        results.lifecycleEvents.push({ name, payload });
      }
      if (name === 'window.close_requested' && payload?.requestId) {
        if (payload?.reason === 'app-exit') {
          render();
          return;
        }
        if (payload?.reason === 'command') {
          render();
          return;
        }
        const shouldPrevent = preventCloseCheckbox?.checked === true;
        const command = shouldPrevent ? 'window.prevent_close' : 'window.confirm_close';
        if (!results.manualCloseDecision && bridge.commands.includes(command)) {
          results.allowedCalls.lastCloseDecision = await bridge.invoke(command, {
            requestId: payload.requestId,
          });
        }
      }
      render();
    });
  }

  try {
    if (bridge.commands.includes('app.ping')) {
      results.allowedCalls.ping = await bridge.invoke('app.ping', {
        from: 'multi-window',
      });
    }
    if (bridge.commands.includes('app.info')) {
      results.allowedCalls.appInfo = await bridge.invoke('app.info', null);
    }
    if (bridge.commands.includes('app.echo')) {
      results.allowedCalls.appEcho = await bridge.invoke('app.echo', {
        from: 'multi-window',
        async: true,
      });
    }
    if (bridge.commands.includes('window.list')) {
      results.allowedCalls.windowList = await bridge.invoke('window.list', null);
    }
    if (bridge.commands.includes('window.info')) {
      results.windowInfo = await bridge.invoke('window.info', null);
    }
    if (bridge.commands.includes('window.focus')) {
      const target = bridge.commands.includes('window.list') ? 'settings' : undefined;
      results.allowedCalls.windowFocus = await bridge.invoke(
        'window.focus',
        target ? { target } : null,
      );
    }
    if (bridge.commands.includes('window.set_title')) {
      const target = bridge.commands.includes('window.list') ? 'settings' : undefined;
      const title = target
        ? 'Settings · Controlled From Main'
        : `${results.windowInfo?.title ?? 'Settings'} · Controlled`;
      results.allowedCalls.windowTitleUpdate = await bridge.invoke(
        'window.set_title',
        target ? { target, title } : { title },
      );
    }
    if (bridge.commands.includes('window.info') && bridge.commands.includes('window.list')) {
      results.allowedCalls.targetedWindowInfo = await bridge.invoke('window.info', {
        target: 'settings',
      });
    }
    if (closeSettingsButton) {
      closeSettingsButton.disabled = !(
        bridge.commands.includes('window.close') && bridge.commands.includes('window.list')
      );
      closeSettingsButton.addEventListener('click', async () => {
        try {
          results.allowedCalls.closedSettings = await closeTargetWithDecision('settings', 'confirm');
          if (bridge.commands.includes('window.list')) {
            results.allowedCalls.windowListAfterClose = await bridge.invoke('window.list', null);
          }
          render();
        } catch (error) {
          results.deniedProbes['window.close'] =
            error instanceof Error ? error.message : String(error);
          render();
        }
      });
    }
    if (closePreviewButton) {
      closePreviewButton.disabled = !(
        bridge.commands.includes('window.close') && bridge.commands.includes('window.list')
      );
      closePreviewButton.addEventListener('click', async () => {
        try {
          results.allowedCalls.closedPreview = await closeTargetWithDecision('preview', 'confirm');
          if (bridge.commands.includes('window.list')) {
            results.allowedCalls.windowListAfterPreviewClose = await bridge.invoke(
              'window.list',
              null,
            );
          }
          render();
        } catch (error) {
          results.deniedProbes['window.close.preview'] =
            error instanceof Error ? error.message : String(error);
          render();
        }
      });
    }
    if (appExitButton) {
      appExitButton.disabled = !bridge.commands.includes('app.exit');
      appExitButton.addEventListener('click', async () => {
        try {
          results.allowedCalls.appExit = await bridge.invoke('app.exit', null);
          render();
        } catch (error) {
          results.deniedProbes['app.exit'] =
            error instanceof Error ? error.message : String(error);
          render();
        }
      });
    }

    const deniedProbeCommands = [
      'app.ping',
      'app.info',
      'app.echo',
      'app.exit',
      'window.list',
      'window.info',
      'window.close',
      'window.focus',
      'window.set_title',
    ].filter((command) => !bridge.commands.includes(command));
    for (const command of deniedProbeCommands) {
      try {
        await bridge.invoke(command, null);
        results.deniedProbes[command] = 'unexpectedly allowed';
      } catch (error) {
        results.deniedProbes[command] = error instanceof Error ? error.message : String(error);
      }
    }

    if (bridge.events.includes('app.log')) {
      results.allowedCalls.hostLog = await bridge.emit('app.log', {
        message: 'multi-window frontend is ready',
        windowId: results.ready?.windowId ?? results.windowInfo?.id ?? null,
        commands: results.commands,
      });
    }

    render();
  } catch (error) {
    status.textContent = `Axion invoke failed: ${error instanceof Error ? error.message : String(error)}`;
    details.textContent = status.textContent;
  }
});
