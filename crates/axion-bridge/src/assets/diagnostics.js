  function toPrettyJson(value) {
    return JSON.stringify(value, null, 2);
  }

  function errorCodeFromMessage(message) {
    const match = String(message ?? '').match(/^([a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*)+):\s/);
    return match ? match[1] : null;
  }

  function normalizeError(error, fallbackMessage = 'Axion bridge request failed') {
    if (error && typeof error === 'object') {
      const message = typeof error.message === 'string' && error.message.length > 0
        ? error.message
        : fallbackMessage;
      return {
        code: typeof error.code === 'string' && error.code.length > 0
          ? error.code
          : errorCodeFromMessage(message) ?? 'bridge.error',
        message,
        cause: error
      };
    }

    const message = typeof error === 'string' && error.length > 0 ? error : fallbackMessage;
    return {
      code: errorCodeFromMessage(message) ?? 'bridge.error',
      message,
      cause: error ?? null
    };
  }

  function snapshotTextControl(element, detail = null) {
    const active = document.activeElement;
    return {
      targetId: element?.id ?? null,
      activeElementId: active instanceof HTMLElement ? active.id || active.tagName : null,
      selectionStart: typeof element?.selectionStart === 'number' ? element.selectionStart : null,
      selectionEnd: typeof element?.selectionEnd === 'number' ? element.selectionEnd : null,
      valueLength: typeof element?.value === 'string' ? element.value.length : null,
      scrollLeft: typeof element?.scrollLeft === 'number' ? element.scrollLeft : null,
      scrollTop: typeof element?.scrollTop === 'number' ? element.scrollTop : null,
      detail,
      devicePixelRatio: window.devicePixelRatio ?? null
    };
  }

  function describeBridge() {
    return {
      ready: true,
      appName: state.appName,
      commands: [...state.commands],
      events: [...state.events],
      hostEvents: [...state.hostEvents],
      trustedOrigins: [...state.trustedOrigins],
      protocol: state.protocol,
      version: state.version,
      diagnosticsReportSchema: state.diagnosticsReportSchema,
      currentOrigin: currentOrigin(),
      locationHref: window.location.href
    };
  }
