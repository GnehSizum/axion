(function() {
  if (window.__AXION__) return;
  const configuration = /* AXION_CONFIG */ null;
  const state = Object.freeze({
    ...configuration,
    commands: Object.freeze(configuration.commands),
    events: Object.freeze(configuration.events),
    hostEvents: Object.freeze(configuration.hostEvents),
    trustedOrigins: Object.freeze(configuration.trustedOrigins)
  });
  const listeners = new Map();
  const lastEvents = new Map();

  function currentOrigin() {
    const url = new URL(window.location.href);
    return `${url.protocol}//${url.host}`;
  }

  if (!state.trustedOrigins.includes(currentOrigin())) return;

  function isListenableEvent(event) {
    return state.events.includes(event) || state.hostEvents.includes(event);
  }

  function dispatch(event, payload) {
    if (!isListenableEvent(event)) return false;
    lastEvents.set(event, payload);
    const handlers = listeners.get(event);
    if (handlers) {
      for (const handler of handlers) {
        try { handler(payload); } catch (error) { console.error('Axion listener error', error); }
      }
    }
    window.dispatchEvent(new CustomEvent(`axion:${event}`, { detail: payload }));
    return true;
  }

  function nextRequestId() {
    return `axion_${Date.now().toString(36)}_${Math.random().toString(16).slice(2)}`;
  }

  function bridgeUrl(kind, name, payload, requestId) {
    const encodedName = encodeURIComponent(name);
    const payloadJson = encodeURIComponent(JSON.stringify(payload ?? null));
    const encodedId = encodeURIComponent(requestId);
    return `${state.protocol}://app/__axion__/${kind}/${encodedName}?payload=${payloadJson}&id=${encodedId}`;
  }

  function createBridgeError(errorLike, fallbackMessage) {
    const normalized = normalizeError(errorLike, fallbackMessage);
    const error = new Error(normalized.message);
    error.code = normalized.code;
    error.details = normalized;
    return error;
  }

  async function bridgeFetch(kind, name, payload) {
    const requestId = nextRequestId();
    const response = await fetch(bridgeUrl(kind, name, payload, requestId), {
      headers: { 'X-Axion-Bridge-Token': state.bridgeToken }
    });
    const envelope = await response.json();
    if (envelope.id && envelope.id !== requestId) {
      throw createBridgeError({ code: 'bridge.request-id-mismatch', message: `Axion bridge returned an unexpected request id for ${name}` });
    }
    if (!response.ok || envelope.ok === false) {
      throw createBridgeError(envelope.error, `Axion bridge request failed: ${name}`);
    }
    return envelope.payload;
  }

  async function invoke(command, payload) {
    if (!state.commands.includes(command)) {
      throw createBridgeError({ code: 'bridge.command-not-allowed', message: `Axion command is not allowed: ${command}` });
    }

    return bridgeFetch('invoke', command, payload);
  }

  async function emit(event, payload) {
    if (!state.events.includes(event)) {
      throw createBridgeError({ code: 'bridge.event-not-allowed', message: `Axion event is not allowed: ${event}` });
    }

    await bridgeFetch('emit', event, payload);
    dispatch(event, payload);
    return true;
  }

  function listen(event, handler) {
    if (!isListenableEvent(event)) {
      throw createBridgeError({ code: 'bridge.event-not-listenable', message: `Axion event is not listenable: ${event}` });
    }
    if (typeof handler !== 'function') {
      throw createBridgeError({ code: 'bridge.invalid-listener', message: 'Axion listen() requires a function handler' });
    }
    const handlers = listeners.get(event) || new Set();
    handlers.add(handler);
    listeners.set(event, handlers);
    if (lastEvents.has(event)) {
      handler(lastEvents.get(event));
    }
    return () => {
      const current = listeners.get(event);
      if (!current) return;
      current.delete(handler);
      if (current.size === 0) listeners.delete(event);
    };
  }

  /* AXION_COMPAT_HELPERS */
  /* AXION_DIAGNOSTICS_HELPERS */
  window.__AXION__ = Object.freeze({
    ready: true,
    appName: state.appName,
    commands: state.commands,
    events: state.events,
    hostEvents: state.hostEvents,
    trustedOrigins: state.trustedOrigins,
    protocol: state.protocol,
    version: state.version,
    compat: Object.freeze({
      installTextInputSelectionPatch
    }),
    diagnostics: Object.freeze({
      reportSchema: state.diagnosticsReportSchema,
      currentOrigin,
      describeBridge,
      snapshotTextControl,
      normalizeError,
      toPrettyJson
    }),
    invoke,
    emit,
    listen,
    __dispatchFromHost(token, event, payload) {
      if (token !== state.bridgeToken || !state.hostEvents.includes(event)) return false;
      return dispatch(event, payload);
    }
  });
})();
