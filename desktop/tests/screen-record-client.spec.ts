import { test, expect } from '@playwright/test';
import { RecordingSession, type RecordingInvoke, type RecordingStatus } from '../src/lib/screen-record';
const id = 'a'.repeat(32);
const state = (phase: RecordingStatus['phase'], key = id): RecordingStatus => ({id:key,phase,recordedMillis:0,frames:0,message:null,metadata:null,region:null});
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>(done => { resolve = done; });
  return {promise,resolve};
}
function bridge(run:(command:string,args?:Record<string,unknown>)=>Promise<unknown>):RecordingInvoke {
  return async <T>(command:string,args?:Record<string,unknown>)=>await run(command,args) as T;
}
test('close during reservation waits then cancels without opening portal',async()=>{
  const gate=deferred(),calls:string[]=[];
  const session=new RecordingSession(id,bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_begin'){await gate.promise;return state('reserved');}
  }));
  const opening=session.begin(),closing=session.dispose();
  expect(session.dispose()).toBe(closing);
  expect(calls).toEqual(['screen_record_begin']);
  gate.resolve();expect(await opening).toBeNull();await closing;
  expect(calls).toEqual(['screen_record_begin','screen_record_cancel']);
});
test('lost begin and start replies recover same capture without retries',async()=>{
  const calls:string[]=[];let phase:RecordingStatus['phase']='reserved';
  const session=new RecordingSession(id,bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_begin')throw new Error('lost begin');
    if(command==='screen_record_start'){phase='recording';throw new Error('lost start');}
    if(command==='screen_record_status')return state(phase);
  }));
  expect((await session.begin())?.phase).toBe('recording');
  expect((await session.begin())?.id).toBe(id);await session.dispose();
  expect(calls).toEqual(['screen_record_begin','screen_record_status','screen_record_start','screen_record_status','screen_record_cancel']);
});
test('foreign status cannot recover rejected admission',async()=>{
  const calls:string[]=[];
  const session=new RecordingSession(id,bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_begin')throw new Error('busy');
    if(command==='screen_record_status')return state('recording','b'.repeat(32));
  }));
  await expect(session.begin()).rejects.toThrow('busy');await session.dispose();
  expect(calls).not.toContain('screen_record_start');
});
test('late status suppressed after close; cleanup failure can be retried',async()=>{
  const gate=deferred();let cancels=0,reads=0;
  const session=new RecordingSession(id,bridge(async command=>{
    if(command==='screen_record_status'){if(++reads===2)await gate.promise;return state('recording');}
    if(command==='screen_record_cancel' && ++cancels===1)throw new Error('lost cancel');
  }));
  await session.begin();const refresh=session.refresh();
  await expect(session.dispose()).rejects.toThrow('lost cancel');gate.resolve();
  expect(await refresh).toBeNull();await session.dispose();expect(cancels).toBe(2);
  await expect(session.control('resume')).rejects.toThrow('closed');
});
test('lost pause acknowledgement surfaced without replaying control',async()=>{
  let controls=0;
  const session=new RecordingSession(id,bridge(async command=>{
    if(command==='screen_record_control'){controls++;throw new Error('lost pause');}
    if(command==='screen_record_status')return state('paused');
  }));
  await session.begin();await expect(session.control('pause')).rejects.toThrow('lost pause');
  expect((await session.refresh())?.phase).toBe('paused');expect(controls).toBe(1);await session.dispose();
});

test('close during initial status cannot publish a late recording state',async()=>{
  const gate=deferred(),entered=deferred();
  const session=new RecordingSession(id,bridge(async command=>{
    if(command==='screen_record_status'){entered.resolve();await gate.promise;return state('recording');}
  }));
  const opening=session.begin();await entered.promise;
  const closing=session.dispose();gate.resolve();
  expect(await opening).toBeNull();await closing;
});

test('close waits for admission without starting preparation',async()=>{
  const {RecordingReview}=await import('../src/lib/screen-record');
  const gate=deferred(),calls:string[]=[];
  const review=new RecordingReview(id,'b'.repeat(32),bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_admit')await gate.promise;
    if(command==='media_status')return {id,phase:'uploading',key:null,message:null};
  }));
  const preparing=review.prepare({name:'capture.mp4',fit:'contain',cut:null});
  const closing=review.dispose();expect(calls).toEqual(['screen_record_admit']);
  gate.resolve();await preparing;await closing;
  expect(calls).toEqual(['screen_record_admit','media_status','media_cancel']);
});
test('lost admission recovers; cancellation interrupts pending preparation',async()=>{
  const {RecordingReview}=await import('../src/lib/screen-record');
  const gate=deferred(),entered=deferred(),calls:string[]=[];
  const review=new RecordingReview(id,'b'.repeat(32),bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_admit')throw new Error('lost admission');
    if(command==='media_status')return {id,phase:'encoding',key:null,message:null};
    if(command==='screen_record_prepare'){entered.resolve();await gate.promise;}
    if(command==='media_cancel')gate.resolve();
  }));
  const preparing=review.prepare({name:'capture.mp4',fit:'cover',cut:null});
  await entered.promise;await review.dispose();await preparing;
  expect(calls).toEqual(['screen_record_admit','media_status','screen_record_prepare','media_status','media_cancel']);
  expect(calls).not.toContain('media_confirm');
});
test('foreign admission status is neither prepared nor cancelled',async()=>{
  const {RecordingReview}=await import('../src/lib/screen-record');
  const calls:string[]=[];
  const review=new RecordingReview(id,'b'.repeat(32),bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_admit')throw new Error('busy');
    if(command==='media_status')return {id:'c'.repeat(32),phase:'encoding'};
  }));
  await expect(review.prepare({name:'capture.mp4',fit:'contain',cut:null})).rejects.toThrow('busy');
  await review.dispose();expect(calls).not.toContain('screen_record_prepare');expect(calls).not.toContain('media_cancel');
});
test('explicit confirm required; close preserves dispatched publication',async()=>{
  const {RecordingReview}=await import('../src/lib/screen-record');
  let phase='uploading';const calls:string[]=[];
  const review=new RecordingReview(id,'b'.repeat(32),bridge(async command=>{
    calls.push(command);
    if(command==='screen_record_prepare')phase='review_ready';
    if(command==='media_status')return {id,phase,key:null,message:null};
    if(command==='media_confirm'){phase='publishing';throw new Error('lost confirmation');}
  }));
  await review.prepare({name:'capture.mp4',fit:'contain',cut:null});expect(calls).not.toContain('media_confirm');
  await expect(review.confirm()).rejects.toThrow('lost confirmation');
  expect((await review.refresh())?.phase).toBe('publishing');await review.dispose();
  expect(calls.filter(c=>c==='media_confirm')).toHaveLength(1);expect(calls).not.toContain('media_cancel');
});
