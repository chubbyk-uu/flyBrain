import {createSceneRenderer} from './scene.js';
import {interpolatePoses} from './native-pose-buffer.js';
import {NeuralActivity} from './neural-activity.js';
import * as THREE from 'three';
const stage=document.querySelector('#stage'),out=document.querySelector('#output'),ctx=out.getContext('2d');
stage.style.width='1080px';stage.style.height='1920px';
const renderer=createSceneRenderer(stage);renderer.renderer.setPixelRatio(1);
const filmFill=new THREE.DirectionalLight(0xfff5e8,1.8);filmFill.visible=false;renderer.scene.add(filmFill);renderer.scene.add(filmFill.target);
let frames,scene,shot,wide,W,H,SW,SH;
const eye=document.createElement('canvas');eye.width=450;eye.height=256;
const graph=document.createElement('canvas');graph.width=880;graph.height=270;const activity=new NeuralActivity(graph,null);
const isData=()=>['retina','neural'].includes(shot.view);
window.filmLoad=async(r,s,landscape)=>{
 frames=r.display_replay.frames;scene=r.display_replay.scene;shot=s;wide=landscape;W=wide?1920:1080;H=wide?1080:1920;
 out.width=W;out.height=H;SW=isData()&&wide?1080:W;SH=isData()&&!wide?1100:H;
 stage.style.width=SW+'px';stage.style.height=SH+'px';renderer.resize();renderer.resetTelemetry();renderer.setScene(scene);
 await new Promise(resolve=>setTimeout(resolve,600));return frames.length;
};
function text(t,x,y,size=38,color='#fff',alpha=1){ctx.save();ctx.globalAlpha=alpha;ctx.font=`600 ${size}px 'Microsoft YaHei',sans-serif`;ctx.fillStyle=color;ctx.shadowColor='#000b';ctx.shadowBlur=8;ctx.fillText(t,x,y);ctx.restore();}
window.filmFrame=f=>{
 const time=shot.start+shot.span*f;
 let lo=0,hi=frames.length-1;while(lo+1<hi){const m=(lo+hi)>>1;if(frames[m].snapshot.time_seconds<=time)lo=m;else hi=m;}
 const a=frames[lo],b=frames[hi];let alpha=(time-a.snapshot.time_seconds)/(b.snapshot.time_seconds-a.snapshot.time_seconds);alpha=Math.max(0,Math.min(1,alpha||0));
 // Stereo inserts use one entire synchronous native capture, never independent clocks.
 if(isData())alpha=0;
 const root=a.snapshot.root_position.map((v,k)=>v+(b.snapshot.root_position[k]-v)*alpha);
 const snap={...a.snapshot,time_seconds:isData()?a.snapshot.time_seconds:time,root_position:root};
 renderer.updateFrame(interpolatePoses(a.poses,b.poses,alpha,scene.bodyCount),snap);
 // This is an offline display: native synchronized eye images are already recorded.
 renderer.pendingVision=false;
 const bi=scene.bodies.findIndex(b=>b.name==='fly/c_thorax');const [qw,qx,qy,qz]=a.poses.slice(bi*7+3,bi*7+7);
 const heading=Math.atan2(2*(qx*qy+qw*qz),1-2*(qy*qy+qz*qz));
 const scale=wide?.80:1.20;let off;
 if(shot.view==='world')off=wide?[180,-225,150]:[190,-290,255];
 else {const angle=heading+(shot.angle??(shot.view==='front'?-.28:shot.view==='retina'?2.7:-1.10));const d=(shot.distance??11)*scale*(1-.045*f);off=[Math.cos(angle)*d,Math.sin(angle)*d,(shot.height??4)*scale];}
 const target=shot.view==='world'?[0,0,28]:root;
 renderer.setObserverView(target.map((v,k)=>v+off[k]),target);
 filmFill.visible=!!shot.filmLighting;renderer.renderer.toneMappingExposure=shot.filmLighting?1.28:1;
 if(filmFill.visible){filmFill.position.copy(renderer.camera.position);filmFill.target.position.fromArray(root);}
 renderer.render(snap.time_seconds*1000);
 ctx.fillStyle='#10191c';ctx.fillRect(0,0,W,H);ctx.drawImage(stage,0,0,SW,SH);
 let fade=ctx.createLinearGradient(0,H*.76,0,H);fade.addColorStop(0,'#0000');fade.addColorStop(1,'#000a');ctx.fillStyle=fade;ctx.fillRect(0,H*.76,W,H*.24);
 if((shot.name==='hook'||shot.name==='title')&&f<.7){const opacity=Math.min(1,f*18,(.7-f)*12);text('我在电脑里',wide?100:70,wide?125:190,wide?66:72,'#fff',opacity);text('养了一只赛博果蝇',wide?100:70,wide?215:290,wide?76:84,'#8ff5dc',opacity);}
 else if(shot.key&&f<.38){text(shot.key,wide?90:65,wide?105:160,wide?45:48,'#a4f5df',Math.min(1,f*22,(.38-f)*20));}
 if(shot.view==='world'){
  const p=renderer.camera.position.clone().fromArray(root).project(renderer.camera),x=(p.x+1)*SW/2,y=(1-p.y)*SH/2;
  ctx.strokeStyle='#8ff5dc';ctx.lineWidth=3;ctx.beginPath();ctx.arc(x,y,18,0,Math.PI*2);ctx.moveTo(x+18,y-8);ctx.lineTo(x+68,y-55);ctx.stroke();text('它在这里',x+75,y-55,wide?29:32,'#b7ffeb');
 }
 if(isData()){
  const x=wide?1130:70,y=wide?230:1080,pw=wide?720:940;
  if(shot.view==='retina'){
   const bytes=Uint8Array.from(atob(a.stereo_base64),c=>c.charCodeAt(0));const rgba=new Uint8ClampedArray(bytes.length*4);
   bytes.forEach((v,i)=>{rgba[i*4]=rgba[i*4+1]=rgba[i*4+2]=v;rgba[i*4+3]=255;});eye.getContext('2d').putImageData(new ImageData(rgba,450,256),0,0);
   text('左眼',x+pw*.18,y-30,wide?34:38);text('右眼',x+pw*.69,y-30,wide?34:38);
   ctx.imageSmoothingEnabled=false;ctx.drawImage(eye,x,y,pw,pw*256/450);ctx.imageSmoothingEnabled=true;
  }else{
   text('神经网络，正在运行',x,y-35,wide?44:48,'#aff7e2');
   activity.reset();for(let i=0;i<=lo;i++)activity.push(frames[i].snapshot);
   ctx.drawImage(graph,x,y,pw,pw*270/880);
   text(`${Math.round(snap.filtered_population_rate_hz).toLocaleString()} 次放电 / 秒`,x,y+pw*270/880+75,wide?37:46,'#8ff5dc');
   text('真实群体活动',x,y+pw*270/880+130,wide?29:32,'#b9c9c9');
  }
 }
 return out.toDataURL('image/jpeg',.95).split(',')[1];
};window.filmReady=true;
