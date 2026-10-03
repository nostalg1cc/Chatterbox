// Production-function regression tests; no live service requests.
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { runInNewContext } from 'node:vm';
import assert from 'node:assert/strict';
const source = readFileSync(new URL('../src/stores/voice.ts', import.meta.url), 'utf8');
const between = (start, end) => { const offset=source.indexOf(start); assert(offset>=0,start); const finish=source.indexOf(end,offset+start.length); assert(finish>=0,end); return source.slice(offset,finish); };
const deferred = () => { let resolve; const promise=new Promise(r=>{resolve=r;}); return {promise,resolve}; };
const compile = (ts,globals) => runInNewContext(stripTypeScriptTypes(ts),globals);
{
 const rpc=deferred(),started=deferred(); let state={status:'idle',activeConversationId:null,sessionId:null};
 const captured={},newerMicrophone={},stopped=[],leaves=[];
 const globals={pendingJoinMicrophones:new Map(),joinAttempt:0,currentUserId:'user',desiredConversationId:null,microphone:newerMicrophone,
 get:()=>state,set:value=>{state={...state,...(typeof value==='function'?value(state):value)};},
 crypto:{randomUUID:()=> 'old-session'},AbortSignal,recordVoiceEvent(){},createPreferredMicrophone:async()=>captured,
 stopMicrophonePipeline:async pipeline=>stopped.push(pipeline),queueMembership:task=>task(),
 supabase:{rpc:(name,body)=>({abortSignal:()=>{if(name==='leave_voice_room'){leaves.push(body.p_session_id);return Promise.resolve({});}started.resolve();return rpc.promise;}})},console};
 const join=compile('('+between('  join: async ','\n  leave: async').replace(/^  join: /,'').trim().replace(/,$/,'')+')',globals);
 const pending=join('room');await started.promise;globals.joinAttempt++;globals.desiredConversationId='new-room';
 state={status:'connected',activeConversationId:'new-room',sessionId:'new-session'};rpc.resolve({error:null,data:{status:'joined'}});await pending;
 assert.equal(state.sessionId,'new-session');assert.equal(globals.microphone,newerMicrophone);assert.deepEqual(stopped,[captured]);assert.deepEqual(leaves,['old-session']);
 console.log('PASS: canceled join cannot resurrect or stop a newer session');
}
for(const stale of [true,false]){
 const rpc=deferred();let state={activeConversationId:'room',sessionId:'old-session',sharingScreen:false};let recovered=0;
 const globals={leaseRequest:null,useVoice:{getState:()=>state},AbortSignal,recordVoiceEvent(){},supabase:{rpc:()=>({abortSignal:()=>rpc.promise})},sameVoiceSession:(session,room)=>state.sessionId===session&&state.activeConversationId===room,recoverMembership:async()=>{recovered++;},console};
 const heartbeat=compile(between('async function sendHeartbeat()','\nasync function recoverMembership')+'\nsendHeartbeat',globals);
 const pending=heartbeat();if(stale)state={...state,sessionId:'new-session'};rpc.resolve({error:null,data:{status:'not_found'}});await pending;assert.equal(recovered,stale?0:1);
 console.log(stale?'PASS: stale heartbeat cannot affect a newer session':'PASS: expired active lease requests recovery');
}
{
 let closed=false,timer;const globals={roomChannel:{presenceState:()=>({})},currentUserId:'local',peerConnection:{connectionState:'connected'},presenceGraceTimer:null,closePeerConnection:()=>{closed=true;},setTimeout:callback=>{timer=callback;return 1;},useVoice:{getState:()=>({activeConversationId:'room',participants:{}}),setState(){}}};
 const sync=compile(between('function syncRoomPresence()','\nasync function reconcileRemoteParticipant')+'\nsyncRoomPresence',globals);sync();assert.equal(closed,false);assert.equal(typeof timer,'function');
 console.log('PASS: missing signaling presence preserves healthy media');
}
{
 let closed=false,cleared=false;const globals={screenSubscribeAttempt:0,screenSubscriberAbort:null,subscriberRenewalTimer:null,remoteScreen:{},screenSubscriber:{close:()=>{closed=true;}},useVoice:{setState:value=>{cleared=value.remoteScreenStream===null;}}};
 const clear=compile(between('function clearRemoteScreen()','\nfunction watchScreenConnection')+'\nclearRemoteScreen',globals);clear();assert(closed&&cleared);assert.equal(globals.screenSubscriber,null);assert.equal(globals.screenSubscribeAttempt,1);assert(between('async function disconnectLocal(','\ntype VoiceDataMessage').includes('clearRemoteScreen();'));
 console.log('PASS: viewer teardown closes and invalidates subscriber');
}
{
 let allocated=false;const globals={currentUserId:'local',useVoice:{getState:()=>({activeConversationId:'room',sessionId:'self',rooms:{room:{generation:'generation'}},participants:{room:[{user_id:'remote',session_id:'valid'}]}})},ensurePeerConnection:()=>{allocated=true;}};
 const handle=compile(between('async function handleSignal(','\nasync function flushPendingCandidates')+'\nhandleSignal',globals);await handle({version:1,generation:'generation',fromSessionId:'forged',type:'ready'});assert.equal(allocated,false);
 console.log('PASS: signaling rejects a sender outside the authoritative seat');
}
