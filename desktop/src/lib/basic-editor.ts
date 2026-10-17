import type { BasicBackground, BasicEditorState, BasicOverlay, Theme } from './api';
import { lcdPageKind } from './lcd-pages';
import { ValidationError } from './validation-error';
export const basicOverlays:readonly BasicOverlay[]=['none','slider_sensor','duo_sensor','three_sensor','four_sensor','time','custom'];
/** Admission of cache structure only. Native validation still owns asset/style limits. */
function editorState(theme:Theme):BasicEditorState {
  const editor=theme.basic_editor;
  if(!editor || lcdPageKind(theme)!=='basic')throw new ValidationError('В этой теме нет сохранённых вариантов фона и текста.');
  const backgrounds=editor.banks.map(bank=>bank.background);
  if(!backgrounds.length || backgrounds.length>2 || new Set(backgrounds).size!==backgrounds.length || !backgrounds.includes(editor.selected_background))throw new ValidationError('Сохранённые варианты фона повреждены. Загрузите тему заново.');
  for(const bank of editor.banks) {
    const overlays=bank.overlays.map(item=>item.overlay);
    if(!['image_video','background_color'].includes(bank.background) || overlays.length!==7 || new Set(overlays).size!==7 || !overlays.every(kind=>basicOverlays.includes(kind)) || !overlays.includes(bank.selected_overlay) || bank.overlays.some(item=>item.theme.basic_editor || item.theme.page_cache?.length || lcdPageKind(item.theme)!=='basic'))throw new ValidationError('Сохранённые варианты текста повреждены. Загрузите тему заново.');
  }
  return editor;
}
/** Local draft operation. Preserve active edits before selecting a saved template. */
export function selectBasicEditor(theme:Theme,background:BasicBackground,overlay:BasicOverlay):Theme {
  editorState(theme);
  const current=structuredClone(theme),editor=current.basic_editor!;
  delete current.basic_editor;delete current.page_cache;
  const active=editor.banks.find(bank=>bank.background===editor.selected_background)!;
  const isMark=(layer:Theme['layers'][number])=>layer.kind==='image' && !!layer.watermark_position;
  const marks=current.layers.filter(isMark);
  for(const item of active.overlays) {
    item.theme.version=current.version;item.theme.background=structuredClone(current.background);
    for(const field of ['background_image','background_video','background_fill'] as const) {
      if(current[field]===undefined)delete item.theme[field];
      else Object.assign(item.theme,{[field]:structuredClone(current[field])});
    }
    item.theme.layers=[...item.theme.layers.filter(layer=>!isMark(layer)),...structuredClone(marks)];
    if(item.theme.layers.length>32)throw new ValidationError('Слишком много слоёв для переключения. Удалите лишние водяные знаки и повторите.');
  }
  active.overlays.find(item=>item.overlay===active.selected_overlay)!.theme=current;
  const target=editor.banks.find(bank=>bank.background===background);
  const selected=target?.overlays.find(item=>item.overlay===overlay);
  if(!target || !selected)throw new ValidationError('Этот вариант не сохранён в теме. Выберите доступный вариант.');
  target.selected_overlay=overlay;editor.selected_background=background;
  const next=structuredClone(selected.theme);
  next.name=theme.name;
  next.basic_editor=editor;
  if(theme.page_cache)next.page_cache=structuredClone(theme.page_cache);
  return next;
}
