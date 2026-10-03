import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import {runInNewContext} from 'node:vm';
import assert from 'node:assert/strict';
const source=readFileSync(new URL('../src/lib/cloudflare-realtime.ts',import.meta.url),'utf8').replace(/^import .*;\r?\n/gm,'').replace(/export /g,'');
for(const kind of ['Publisher','Subscriber']){
 let resolve, startedResolve;const pending=new Promise(r=>resolve=r),started=new Promise(r=>startedResolve=r);let peer;
 class PC{constructor(){peer=this;this.connectionState='new';this.iceGatheringState='complete';this.localDescription={toJSON:()=>({type:'offer',sdp:'offer'})};}async createOffer(){return{};}async setLocalDescription(){}async setRemoteDescription(){if(this.connectionState==='closed')throw new Error('closed');}close(){this.connectionState='closed';}}
 const globals={RTCPeerConnection:PC,MediaStream:class{},supabase:{functions:{invoke:()=>{startedResolve();return pending;}}},SCREEN_SHARE_MAX_BITRATE:1000000};
 const factories=runInNewContext(stripTypeScriptTypes(source+'\n({createCloudflareScreenPublisher,createCloudflareScreenSubscriber})'),globals);
 const controller=new AbortController();const args=kind==='Publisher'?['room',{getTracks:()=>[]},'voice',[],controller.signal]:['room','remote',['track'],()=>{},'voice',[],controller.signal];
 const task=factories['createCloudflareScreen'+kind](...args);await started;controller.abort();assert.equal(peer.connectionState,'closed');resolve({data:{sessionId:'provider',sessionDescription:{type:'answer',sdp:'answer'}}});await assert.rejects(task);
 console.log('PASS: pending screen '+kind.toLowerCase()+' closes immediately on cancellation');
}
