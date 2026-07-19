const MESSAGE_PREVIEW_LENGTH = 120;

export function messagePreview(text: string) {
  const firstLine = text.split(/\r?\n/, 1)[0]?.trim() ?? "";
  const preview = firstLine.slice(0, MESSAGE_PREVIEW_LENGTH).trimEnd();
  return preview.length < firstLine.length || text.trim() !== firstLine ? `${preview}…` : preview;
}
