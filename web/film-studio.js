import {createSceneRenderer} from './scene.js';
import {interpolatePoses} from './native-pose-buffer.js';
import {NeuralActivity} from './neural-activity.js';
import {decodeRetinaPreview,drawRetinaPreview} from './native-retina.js';
const stage=document.querySelector('#stage'),output=document.querySelector('#output'),ctx=output.getContext('2d');
const renderer=createSceneRenderer(stage);renderer.renderer.setPixelRatio(1);renderer.resize();
const graph=document.createElement('canvas');graph.width=920;graph.height=260;const neural=new NeuralActivity(graph,null);
const retina=document.createElement('canvas');
let report,frames,shot,live;
window.filmLoad=async(r,s,l)=>{report=r;frames=r.display_replay.frames;shot=s;live=l;renderer.setScene(r.display_replay.scene);neural.reset();await new Promise(resolve=>setTimeout(resolve,600));return frames.length;};
function text(t,x,y,size=36,color='#ecf5f4'){ctx.fillStyle=color;ctx.font=`${size>=54?'bold ':''}${size}px 'Microsoft YaHei',sans-serif`;ctx.fillText(t,x,y);}
window.filmFrame=(fraction)=>{
 const t=shot.start+fraction*shot.span;
 let i=Math.max(0,Math.min(frames.length-2,Math.floor(t*50)));
 while(i>0&&frames[i].snapshot.time_seconds>t)i--;
 while(i<frames.length-2&&frames[i+1].snapshot.time_seconds<t)i++;
 const a=frames[i],b=frames[i+1],alpha=Math.max(0,Math.min(1,(t-a.snapshot.time_seconds)/(b.snapshot.time_seconds-a.snapshot.time_seconds)));
 const root=a.snapshot.root_position.map((v,k)=>v+(b.snapshot.root_position[k]-v)*alpha);
 const snap={...a.snapshot,time_seconds:t,root_position:root};
 renderer.updateFrame(interpolatePoses(a.poses,b.poses,alpha,report.display_replay.scene.bodyCount),snap);
 let off=shot.offset??[9,-12,5];
 if(shot.view==='front'||shot.view==='side'){
  const bi=report.display_replay.scene.bodies.findIndex(b=>b.name==='fly/c_thorax');const [w,x,y,z]=a.poses.slice(bi*7+3,bi*7+7);
  const heading=Math.atan2(2*(x*y+w*z),1-2*(y*y+z*z));
  const angle=heading+(shot.view==='front'?-.25:-1.1);
  const d=shot.distance??9;off=[d*Math.cos(angle),d*Math.sin(angle),shot.height??3.5];
 }
 const zoom=1-.07*fraction;off=off.map(v=>v*zoom);
 const target=shot.view==='world'?[0,0,28]:root;
 renderer.setObserverView(target.map((v,k)=>v+off[k]),target);renderer.render(t*1000);
 ctx.fillStyle='#07111b';ctx.fillRect(0,0,1080,1920);
 ctx.save();ctx.beginPath();ctx.rect(0,250,1080,1420);ctx.clip();ctx.drawImage(stage,0,['retina','neural'].includes(shot.view)?-30:250,1080,1420);ctx.restore();
 let g=ctx.createLinearGradient(0,1340,0,1740);g.addColorStop(0,'#07111b00');g.addColorStop(1,'#07111b');ctx.fillStyle=g;ctx.fillRect(0,1340,1080,430);
 text('一个人的数字小世界',64,78,28,'#73d8c3');text(shot.title,60,165,66);text(shot.label,64,218,28,'#a2b8c4');
 text('赛博果蝇观察日记',64,1865,28,'#95acba');text('真实仿真轨迹 · 镜头回放',650,1865,24,'#95acba');
 if(shot.view==='world'){
  const p=renderer.camera.position.clone().fromArray(root).project(renderer.camera),x=(p.x+1)*540,y=(1-p.y)*710+250;
  ctx.strokeStyle='#73e8ce';ctx.lineWidth=3;ctx.beginPath();ctx.arc(x,y,20,0,Math.PI*2);ctx.moveTo(x+20,y-8);ctx.lineTo(x+75,y-65);ctx.stroke();text('它在这里',x+80,y-65,30,'#73e8ce');
 }
 if(shot.view==='retina'){
  const idx=Math.floor(fraction*(live.retina.length-1));const item=live.retina[idx];
  const bytes=Uint8Array.from(atob(item.data),c=>c.charCodeAt(0));drawRetinaPreview(retina,decodeRetinaPreview(bytes.buffer));
  ctx.fillStyle='#091522ee';ctx.fillRect(50,1000,980,645);ctx.imageSmoothingEnabled=false;ctx.drawImage(retina,80,1080,920,920*retina.height/retina.width);ctx.imageSmoothingEnabled=true;
  text('左眼',245,1060,34);text('右眼',745,1060,34);text('原生双眼传感画面 · 非真实复眼完整还原',88,1633,26,'#a2b8c4');
 }
 if(shot.view==='neural'){
  const k=Math.floor(fraction*(live.frames.length-1));const s=live.frames[k].snapshot;
  // Feed genuine earlier samples as well so the graph has a full history at cut-in.
  neural.reset();for(let n=Math.max(0,k-300);n<=k;n++)neural.push(live.frames[n].snapshot);
  ctx.fillStyle='#091522f5';ctx.fillRect(50,965,980,635);text('神经网络，正在运行',85,1035,42);
  ctx.drawImage(graph,80,1070);text(`${Math.round(s.filtered_population_rate_hz).toLocaleString()} 次放电 / 秒`,85,1390,44,'#73e8ce');
  text(`累计参与放电：${Number(s.cumulative_spiking_neuron_count).toLocaleString()} 个`,85,1455,32);
  text('真实总量采样 · 平滑曲线 · 不是思想解码',85,1540,26,'#a2b8c4');
 }
 return output.toDataURL('image/jpeg',.95).split(',')[1];
};
window.filmReady=true;
