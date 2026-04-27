window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
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
  };

  window.__AXION_GUI_SMOKE__ = async () => {
    const checks = [];
    const addCheck = (id, status, detail) => checks.push({ id, status, detail });

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
    bridge.listen(name, (payload) => {
      if (name === 'app.ready') {
        results.ready = payload;
      } else if (name.startsWith('window.')) {
        results.lifecycleEvents.push({ name, payload });
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

    const deniedProbeCommands = [
      'app.ping',
      'app.info',
      'app.echo',
      'window.list',
      'window.info',
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
