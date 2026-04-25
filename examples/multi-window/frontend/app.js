window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected.';
    details.textContent = 'Bridge unavailable';
    return;
  }

  try {
    const commands = window.__AXION__.commands;
    const results = {};
    const lifecycleEvents = [];
    [
      'window.created',
      'window.resized',
      'window.close_requested',
      'window.closed',
      'window.redraw_failed',
    ].forEach((name) => {
      window.__AXION__.listen(name, (payload) => {
        lifecycleEvents.push({ name, payload });
        details.textContent = JSON.stringify({ ...results, lifecycleEvents }, null, 2);
      });
    });

    if (commands.includes('app.ping')) {
      results.ping = await window.__AXION__.invoke('app.ping', { from: 'multi-window' });
    }
    if (commands.includes('app.info')) {
      results.appInfo = await window.__AXION__.invoke('app.info', null);
    }
    if (commands.includes('app.echo')) {
      results.appEcho = await window.__AXION__.invoke('app.echo', {
        from: 'multi-window',
        async: true,
      });
    }
    if (commands.includes('window.info')) {
      results.windowInfo = await window.__AXION__.invoke('window.info', null);
    }
    if (window.__AXION__.events.includes('app.log')) {
      results.hostLog = await window.__AXION__.emit('app.log', {
        message: 'multi-window frontend is ready',
        windowInfo: results.windowInfo ?? null,
      });
    }

    status.textContent = `Axion bridge ready with commands: ${commands.join(', ') || 'none'}`;
    details.textContent = JSON.stringify({ ...results, lifecycleEvents }, null, 2);
  } catch (error) {
    status.textContent = `Axion invoke failed: ${error instanceof Error ? error.message : String(error)}`;
    details.textContent = status.textContent;
  }
});
