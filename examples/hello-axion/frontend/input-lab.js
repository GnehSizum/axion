window.addEventListener('DOMContentLoaded', () => {
  const fields = ['lab-input', 'lab-textarea'].map((id) => document.getElementById(id));
  const fieldSnapshot = document.getElementById('field-snapshot');
  const deviceSnapshot = document.getElementById('device-snapshot');
  const eventLog = document.getElementById('event-log');
  const scrollProbe = document.getElementById('scroll-probe');
  const exportText = document.getElementById('diagnostics-json');
  const startedAt = performance.now();
  const events = [];
  const counts = {};
  const composing = new Set();
  const round = (value) => Math.round(value * 100) / 100;
  let sequence = 0;
  let lastClick = null;
  let lastWheel = null;

  function fieldState(field) {
    return {
      id: field.id,
      valueUtf16Length: field.value.length,
      selectionStart: field.selectionStart,
      selectionEnd: field.selectionEnd,
      selectionDirection: field.selectionDirection,
      composing: composing.has(field.id),
      scrollLeft: round(field.scrollLeft),
      scrollTop: round(field.scrollTop)
    };
  }

  function focusTarget() {
    const active = document.activeElement;
    return fields.includes(active) ? active.id : active === scrollProbe ? 'scroll-probe' : 'other';
  }

  function deviceState() {
    return {
      devicePixelRatio: window.devicePixelRatio,
      viewportCssPixels: { width: window.innerWidth, height: window.innerHeight },
      documentFocused: document.hasFocus(),
      focusedTarget: focusTarget(),
      lastClickCssPixels: lastClick,
      lastWheel,
      scrollProbe: { left: round(scrollProbe.scrollLeft), top: round(scrollProbe.scrollTop) }
    };
  }

  function refresh() {
    fieldSnapshot.textContent = JSON.stringify(fields.map(fieldState), null, 2);
    deviceSnapshot.textContent = JSON.stringify(deviceState(), null, 2);
  }

  function record(event, field) {
    if (event.type === 'compositionstart') composing.add(field.id);
    if (event.type === 'compositionend' || event.type === 'blur') composing.delete(field.id);
    counts[event.type] = (counts[event.type] || 0) + 1;
    const entry = {
      sequence: ++sequence,
      elapsedMs: round(performance.now() - startedAt),
      type: event.type,
      target: field.id,
      trusted: event.isTrusted,
      isComposing: typeof event.isComposing === 'boolean' ? event.isComposing : null,
      dataUtf16Length: typeof event.data === 'string' ? event.data.length : null,
      inputType: typeof event.inputType === 'string' ? event.inputType : null,
      keyCategory: typeof event.key === 'string' ? event.key.length === 1 ? 'character' : 'named-or-multiple' : null,
      field: fieldState(field)
    };
    events.push(entry);
    if (events.length > 80) events.shift();
    eventLog.textContent = events.map((entry) => JSON.stringify(entry)).join('\n');
    eventLog.scrollTop = eventLog.scrollHeight;
    refresh();
  }

  for (const field of fields) {
    for (const type of ['compositionstart', 'compositionupdate', 'compositionend', 'beforeinput', 'input', 'keydown', 'keyup', 'select', 'focus', 'blur']) {
      field.addEventListener(type, (event) => record(event, field));
    }
    field.addEventListener('scroll', refresh);
  }
  document.addEventListener('selectionchange', refresh);
  document.addEventListener('mousedown', (event) => {
    lastClick = {
      x: round(event.clientX), y: round(event.clientY),
      target: fields.includes(event.target) ? event.target.id : 'other',
      trusted: event.isTrusted
    };
    refresh();
  });
  scrollProbe.addEventListener('wheel', (event) => {
    lastWheel = { x: round(event.deltaX), y: round(event.deltaY), mode: event.deltaMode, trusted: event.isTrusted };
    refresh();
  }, { passive: true });
  scrollProbe.addEventListener('scroll', refresh);
  window.addEventListener('focus', refresh);
  window.addEventListener('blur', refresh);
  window.addEventListener('resize', refresh);

  function watchPixelRatio() {
    refresh();
    const query = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    query.addEventListener('change', watchPixelRatio, { once: true });
  }
  watchPixelRatio();

  document.getElementById('clear-events').addEventListener('click', () => {
    events.length = 0;
    for (const type of Object.keys(counts)) delete counts[type];
    eventLog.textContent = '尚无输入事件。';
    refresh();
  });
  document.getElementById('export-diagnostics').addEventListener('click', () => {
    exportText.value = JSON.stringify({
      kind: 'axion-input-lab',
      verification: 'manual-not-recorded',
      bridgeAvailable: window.__AXION__?.ready === true,
      device: deviceState(),
      fields: fields.map(fieldState),
      eventCounts: { ...counts },
      recentEvents: events.slice()
    }, null, 2);
    exportText.focus();
    exportText.select();
    document.getElementById('export-status').textContent = 'JSON 已选择；按 Cmd+C / Ctrl+C 复制。页面没有自动写入系统剪贴板，也没有判定硬件验收结果。';
  });
});
