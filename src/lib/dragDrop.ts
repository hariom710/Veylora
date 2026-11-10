// Window-level native drag-drop from Windows Explorer.
//
// Tauri 2 intercepts OS drag-drop (HTML5 drop events never fire in the
// webview), so paths come through the webview event, not the DOM.
// Returns an unsubscribe function. No-op outside a Tauri webview (tests).
export function onFilesDropped(handler: (paths: string[]) => void): () => void {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
    return () => {};
  }
  let disposed = false;
  let unlisten: (() => void) | undefined;
  import('@tauri-apps/api/webview')
    .then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
          handler(event.payload.paths);
        }
      }),
    )
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
    .catch(() => {
      /* drag-drop unavailable — Import button still works */
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}
