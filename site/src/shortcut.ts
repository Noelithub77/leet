export function shouldCopyInstallCommand({ key, ctrlKey, metaKey, altKey, shiftKey, repeat, editable, selection, platform }) {
  return platform !== 'windows'
    && key.toLowerCase() === 'c'
    && (ctrlKey || metaKey)
    && !altKey && !shiftKey && !repeat
    && !editable && !selection;
}
