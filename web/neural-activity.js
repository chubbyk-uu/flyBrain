// Actual native aggregate telemetry, not synthetic neurons or anatomical locations.
export class NeuralActivity {
  constructor(canvas, label) { this.canvas=canvas; this.label=label; this.reset(); }
  reset() { this.points=[]; this.lastTime=-1; }
  push(s) {
    if (!Number.isFinite(s.filtered_population_rate_hz)) return;
    if(s.time_seconds<this.lastTime) this.reset();
    if(s.time_seconds<=this.lastTime) return;
    this.lastTime=s.time_seconds;
    this.points.push([s.time_seconds,s.filtered_population_rate_hz]);
    this.points=this.points.filter(p=>p[0]>=s.time_seconds-10);
    if(this.label) this.label.textContent=`全 CNS ${Math.round(s.filtered_population_rate_hz).toLocaleString()} 次放电/秒 · 累计参与 ${Number(s.cumulative_spiking_neuron_count??0).toLocaleString()} 个神经元`;
    this.draw();
  }
  draw() {
    const c=this.canvas, x=c.getContext('2d'), w=c.width,h=c.height;
    x.fillStyle='#091522';x.fillRect(0,0,w,h);
    x.strokeStyle='#203548';x.lineWidth=1;
    for(let i=1;i<4;i++){x.beginPath();x.moveTo(0,h*i/4);x.lineTo(w,h*i/4);x.stroke();}
    const values=this.points.map(p=>p[1]);
    const high=Math.max(1,...values), low=Math.min(high,...values);
    const range=Math.max(high-low,high*.02,1);
    const min=Math.max(0,low-range*.15),max=high+range*.15;
    x.strokeStyle='#60f1d2';x.lineWidth=3;x.beginPath();
    this.points.forEach(([t,v],i)=>{const px=w*(t-this.lastTime+10)/10,py=h-12-(v-min)/(max-min)*(h-42);i?x.lineTo(px,py):x.moveTo(px,py);});x.stroke();
    x.fillStyle='#9cbdcc';x.font='16px sans-serif';x.fillText(`${Math.floor(min).toLocaleString()} → ${Math.ceil(max).toLocaleString()} 次/秒 · 10 秒 · 自动纵轴`,12,22);
  }
}
