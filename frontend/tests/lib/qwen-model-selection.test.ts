import {expect,mock,test} from 'bun:test';
import * as React from 'react';
import {DEFAULT_QWEN_MODEL,QWEN_MODELS} from '../../src/lib/qwen-models';
let state:any[]=[],cursor=0,refs:any[]=[],r=0;
mock.module('react',()=>({...React,useState:(initial:any)=>{const i=cursor++;if(!(i in state))state[i]=initial;return[state[i],(value:any)=>state[i]=value];},useRef:(initial:any)=>refs[r++]??={current:initial},useCallback:(fn:any)=>fn}));
let native:any[]=[];let fail=false;const calls:string[]=[];
mock.module('@tauri-apps/api/core',()=>({invoke:async(name:string)=>{calls.push(name);if(fail)throw Error('fixture');return native;}}));
const {useTranscriptionModels}=await import('../../src/hooks/useTranscriptionModels');
const small='Qwen3-ASR-0.6B-INT8';
function harness(model:string){state=[];refs=[];calls.length=0;fail=false;return()=>{cursor=0;r=0;return useTranscriptionModels({provider:'qwen',model});};}
test('catalog exposes exactly 1.7B and 0.6B; default stays 1.7B',()=>{expect(QWEN_MODELS.map(m=>m.id)).toEqual([DEFAULT_QWEN_MODEL,small]);});
test('batch dialog discovers both models only via Qwen and restores selected small model',async()=>{native=QWEN_MODELS.map(m=>({name:m.id,status:'Available',size_mb:0}));const render=harness(small);await render().fetchModels();expect(render().availableModels.map(m=>m.provider)).toEqual(['qwen','qwen']);expect(render().selectedModelKey).toBe(`qwen:${small}`);expect(calls).toEqual(['qwen_get_available_models']);});
test('missing configured model is not replaced by the other installed model',async()=>{native=[{name:small,status:'Available'}];const render=harness(DEFAULT_QWEN_MODEL);await render().fetchModels();expect(render().selectedModelKey).toBe('');expect(render().availableModels).toHaveLength(1);});
test('manual selection survives refresh and failed enumeration clears readiness',async()=>{native=QWEN_MODELS.map(m=>({name:m.id,status:'Available'}));const render=harness(DEFAULT_QWEN_MODEL);await render().fetchModels();render().setSelectedModelKey(`qwen:${small}`);await render().fetchModels();expect(render().selectedModelKey).toBe(`qwen:${small}`);fail=true;await render().fetchModels();expect(render().selectedModelKey).toBe('');expect(render().availableModels).toEqual([]);});
