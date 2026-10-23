export interface ModelWithStatus { status?: unknown }
interface ProviderCommands { initialize:string;hasAvailableModels:string;getAvailableModels:string }
export function getProviderCommands(provider:string):ProviderCommands|null {
  return provider==='qwen'?{initialize:'qwen_init',hasAvailableModels:'qwen_has_available_models',getAvailableModels:'qwen_get_available_models'}:null;
}
export function hasDownloadingModel(models:ModelWithStatus[]):boolean {
  return models.some(({status})=>status==='Downloading'||(status!==null&&typeof status==='object'&&'Downloading' in status));
}
