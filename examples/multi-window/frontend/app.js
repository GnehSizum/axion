window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected.';
    details.textContent = 'Bridge unavailable';
    return;
  }

  const bridge = window.__AXION__;
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

  const render = () => {
    const windowLabel =
      results.ready?.windowId ||
      results.windowInfo?.id ||
      (bridge.commands.includes('window.info') ? 'settings-like window' : 'main-like window');
    status.textContent = `Axion multi-window bridge ready: ${windowLabel}`;
    details.textContent = JSON.stringify(results, null, 2);
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
    if (bridge.commands.includes('window.info')) {
      results.windowInfo = await bridge.invoke('window.info', null);
    }

    const deniedProbeCommands = ['app.ping', 'app.info', 'app.echo', 'window.info'].filter(
      (command) => !bridge.commands.includes(command),
    );
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
