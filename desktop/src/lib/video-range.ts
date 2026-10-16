export interface SourceFrameRate { numerator:number; denominator:number }
export interface SourceMetadata {
  width:number; height:number; durationSeconds:number|null;
  averageFrameRate:SourceFrameRate|null; nominalFrameRate:SourceFrameRate|null;
}
export interface VideoEditRange { startSeconds:number; endSeconds:number }
export interface VideoEditBounds { maximumSeconds:number; minimumSpanSeconds:number; fps:number }
export function videoEditBounds(metadata:SourceMetadata|null|undefined):VideoEditBounds|null {
  if(!metadata)return null;
  const duration=metadata.durationSeconds, rate=metadata.averageFrameRate ?? metadata.nominalFrameRate;
  if(duration===null || !Number.isFinite(duration) || duration<=0 || duration>3600 || !rate
    || !Number.isInteger(rate.numerator) || rate.numerator<=0 || !Number.isInteger(rate.denominator) || rate.denominator<=0)return null;
  const tick=rate.denominator/rate.numerator;
  const maximumSeconds=duration-tick, minimumSpanSeconds=2*tick;
  return maximumSeconds>=minimumSpanSeconds ? {maximumSeconds,minimumSpanSeconds,fps:rate.numerator/rate.denominator}:null;
}
export function validVideoRange(start:number|undefined,end:number|undefined,bounds:VideoEditBounds|null):boolean {
  return !!bounds && start!==undefined && end!==undefined && Number.isFinite(start) && Number.isFinite(end)
    && start>=0 && end>start && end<=bounds.maximumSeconds+1e-9 && end-start+1e-9>=bounds.minimumSpanSeconds;
}
