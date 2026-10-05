  function isTextControl(element) {
    return element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement;
  }

  function resolveLineHeight(style) {
    const raw = style.lineHeight;
    if (!raw || raw === 'normal') {
      const fontSize = Number.parseFloat(style.fontSize);
      return Number.isFinite(fontSize) ? fontSize * 1.4 : 18;
    }

    if (raw.endsWith('px')) {
      const value = Number.parseFloat(raw);
      return Number.isFinite(value) ? value : 18;
    }

    const unitless = Number.parseFloat(raw);
    if (!Number.isFinite(unitless)) {
      return 18;
    }

    const fontSize = Number.parseFloat(style.fontSize);
    return Number.isFinite(fontSize) ? unitless * fontSize : unitless;
  }

  function textMetrics(element) {
    const style = window.getComputedStyle(element);
    const canvas = document.createElement('canvas');
    const context = canvas.getContext('2d');
    const font = [
      style.fontStyle,
      style.fontVariant,
      style.fontWeight,
      style.fontSize,
      style.fontFamily
    ].filter(Boolean).join(' ');

    if (context && font) {
      context.font = font;
    }

    return {
      charWidth: context?.measureText('M').width || 8,
      lineHeight: resolveLineHeight(style),
      paddingLeft: Number.parseFloat(style.paddingLeft) || 0,
      paddingTop: Number.parseFloat(style.paddingTop) || 0,
      borderLeft: Number.parseFloat(style.borderLeftWidth) || 0,
      borderTop: Number.parseFloat(style.borderTopWidth) || 0
    };
  }

  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  function caretIndexFromPoint(element, point) {
    if (!isTextControl(element) || typeof point?.clientX !== 'number' || typeof point?.clientY !== 'number') {
      return null;
    }

    const rect = element.getBoundingClientRect();
    const {
      charWidth,
      lineHeight,
      paddingLeft,
      paddingTop,
      borderLeft,
      borderTop
    } = textMetrics(element);
    const relativeX = point.clientX - rect.left - borderLeft - paddingLeft + element.scrollLeft;
    const relativeY = point.clientY - rect.top - borderTop - paddingTop + element.scrollTop;

    if (element instanceof HTMLInputElement) {
      const index = clamp(Math.round(relativeX / charWidth), 0, element.value.length);
      return {
        index,
        detail: {
          kind: 'input',
          relativeX,
          charWidth,
          paddingLeft,
          borderLeft,
          scrollLeft: element.scrollLeft,
          correctedIndex: index
        }
      };
    }

    const lines = element.value.split('\n');
    const lineIndex = clamp(Math.floor(relativeY / lineHeight), 0, Math.max(lines.length - 1, 0));
    const line = lines[lineIndex] ?? '';
    const column = clamp(Math.round(relativeX / charWidth), 0, line.length);
    let absoluteIndex = column;
    for (let index = 0; index < lineIndex; index += 1) {
      absoluteIndex += lines[index].length + 1;
    }

    return {
      index: absoluteIndex,
      detail: {
        kind: 'textarea',
        relativeX,
        relativeY,
        charWidth,
        lineHeight,
        paddingLeft,
        paddingTop,
        borderLeft,
        borderTop,
        scrollLeft: element.scrollLeft,
        scrollTop: element.scrollTop,
        lineIndex,
        column,
        correctedIndex: absoluteIndex
      }
    };
  }

  function normalizeCompatOptions(options) {
    return {
      manualPointerSelection: options?.manualPointerSelection === true,
      onUpdate: typeof options?.onUpdate === 'function' ? options.onUpdate : null,
      onStatus: typeof options?.onStatus === 'function' ? options.onStatus : null
    };
  }

  function reportCompatUpdate(options, element, detail) {
    if (!options.onUpdate) {
      return;
    }

    options.onUpdate({
      targetId: element.id || null,
      selectionStart: typeof element.selectionStart === 'number' ? element.selectionStart : null,
      selectionEnd: typeof element.selectionEnd === 'number' ? element.selectionEnd : null,
      valueLength: typeof element.value === 'string' ? element.value.length : null,
      scrollLeft: typeof element.scrollLeft === 'number' ? element.scrollLeft : null,
      scrollTop: typeof element.scrollTop === 'number' ? element.scrollTop : null,
      detail
    });
  }

  function reportCompatStatus(options, message) {
    if (options.onStatus) {
      options.onStatus(message);
    }
  }

  function setCaretFromPoint(element, point, options, source) {
    const result = caretIndexFromPoint(element, point);
    if (!result) {
      return null;
    }

    element.setSelectionRange(result.index, result.index);
    reportCompatUpdate(options, element, {
      ...result.detail,
      source
    });
    return result.index;
  }

  function setSelectionFromPoint(element, anchorIndex, point, options, source) {
    const result = caretIndexFromPoint(element, point);
    if (!result) {
      return null;
    }

    const currentIndex = result.index;
    const start = Math.min(anchorIndex, currentIndex);
    const end = Math.max(anchorIndex, currentIndex);
    element.setSelectionRange(start, end);
    reportCompatUpdate(options, element, {
      ...result.detail,
      source,
      anchorIndex,
      currentIndex,
      selectionStart: start,
      selectionEnd: end
    });
    return currentIndex;
  }

  function installTextInputSelectionPatch(element, rawOptions) {
    if (!isTextControl(element)) {
      throw new Error('Axion compat patch requires an input or textarea element');
    }

    const options = normalizeCompatOptions(rawOptions);
    const listeners = [];
    const drag = {
      pointerId: null,
      anchorIndex: null,
      pendingPoint: null,
      rafId: null
    };

    function addListener(type, handler) {
      element.addEventListener(type, handler);
      listeners.push(() => element.removeEventListener(type, handler));
    }

    function clearDrag(pointerId = null) {
      if (pointerId !== null && drag.pointerId !== pointerId) {
        return;
      }

      if (drag.rafId !== null) {
        window.cancelAnimationFrame(drag.rafId);
      }

      drag.pointerId = null;
      drag.anchorIndex = null;
      drag.pendingPoint = null;
      drag.rafId = null;
    }

    function queueManualDragSelection(event) {
      drag.pendingPoint = {
        clientX: event.clientX,
        clientY: event.clientY
      };

      if (drag.rafId !== null) {
        return;
      }

      drag.rafId = window.requestAnimationFrame(() => {
        drag.rafId = null;
        if (drag.anchorIndex === null || !drag.pendingPoint) {
          return;
        }

        const currentIndex = setSelectionFromPoint(
          element,
          drag.anchorIndex,
          drag.pendingPoint,
          options,
          'drag-selection-correction'
        );
        if (currentIndex !== null) {
          reportCompatStatus(
            options,
            `Selection corrected: ${element.id || element.tagName}@${drag.anchorIndex}→${currentIndex}`
          );
        }
      });
    }

    if (!options.manualPointerSelection) {
      addListener('click', (event) => {
        window.setTimeout(() => {
          const correctedIndex = setCaretFromPoint(element, event, options, 'caret-correction');
          if (correctedIndex !== null) {
            reportCompatStatus(
              options,
              `Caret corrected after click: ${element.id || element.tagName}@${correctedIndex}`
            );
          }
        }, 0);
      });
      addListener('pointerdown', (event) => {
        window.setTimeout(() => {
          const anchorIndex = setCaretFromPoint(
            element,
            event,
            options,
            'caret-correction'
          );
          if (anchorIndex !== null) {
            drag.pointerId = event.pointerId;
            drag.anchorIndex = anchorIndex;
            reportCompatStatus(
              options,
              `Selection anchor set: ${element.id || element.tagName}@${anchorIndex}`
            );
          }
        }, 0);
      });
      addListener('pointermove', (event) => {
        if (drag.anchorIndex === null || drag.pointerId !== event.pointerId || event.buttons === 0) {
          return;
        }

        window.setTimeout(() => {
          const currentIndex = setSelectionFromPoint(
            element,
            drag.anchorIndex,
            event,
            options,
            'drag-selection-correction'
          );
          if (currentIndex !== null) {
            reportCompatStatus(
              options,
              `Selection corrected: ${element.id || element.tagName}@${drag.anchorIndex}→${currentIndex}`
            );
          }
        }, 0);
      });
      addListener('pointerup', () => clearDrag());
      addListener('pointercancel', () => clearDrag());

      return () => {
        clearDrag();
        for (const dispose of listeners.splice(0)) {
          dispose();
        }
      };
    }

    addListener('pointerdown', (event) => {
      if (event.button !== 0) {
        return;
      }

      event.preventDefault();
      element.focus();

      const anchorIndex = setCaretFromPoint(element, event, options, 'caret-correction');
      if (anchorIndex === null) {
        return;
      }

      drag.pointerId = event.pointerId;
      drag.anchorIndex = anchorIndex;

      if (typeof element.setPointerCapture === 'function') {
        try {
          element.setPointerCapture(event.pointerId);
        } catch (_error) {
        }
      }

      reportCompatStatus(
        options,
        `Manual selection anchor set: ${element.id || element.tagName}@${anchorIndex}`
      );
    });

    addListener('mousedown', (event) => {
      event.preventDefault();
    });

    addListener('mouseup', (event) => {
      event.preventDefault();
    });

    addListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();

      queueMicrotask(() => {
        const correctedIndex = setCaretFromPoint(element, event, options, 'caret-correction');
        if (correctedIndex !== null) {
          reportCompatStatus(
            options,
            `Caret corrected after click: ${element.id || element.tagName}@${correctedIndex}`
          );
        }
      });
    });

    addListener('pointermove', (event) => {
      if (drag.anchorIndex === null || drag.pointerId !== event.pointerId || event.buttons === 0) {
        return;
      }

      event.preventDefault();
      queueManualDragSelection(event);
    });

    addListener('pointerup', (event) => {
      if (drag.anchorIndex === null || drag.pointerId !== event.pointerId) {
        return;
      }

      event.preventDefault();
      const currentIndex = setSelectionFromPoint(
        element,
        drag.anchorIndex,
        event,
        options,
        'drag-selection-correction'
      );
      if (currentIndex !== null) {
        reportCompatStatus(
          options,
          `Selection corrected: ${element.id || element.tagName}@${drag.anchorIndex}→${currentIndex}`
        );
      }

      if (typeof element.releasePointerCapture === 'function') {
        try {
          element.releasePointerCapture(event.pointerId);
        } catch (_error) {
        }
      }

      clearDrag(event.pointerId);
    });

    addListener('pointercancel', (event) => {
      clearDrag(event.pointerId);
    });

    return () => {
      clearDrag();
      for (const dispose of listeners.splice(0)) {
        dispose();
      }
    };
  }
