param(
    [int]$DebugPort = 9227,
    [ValidateRange(10,1800)][int]$DurationSeconds = 600,
    [switch]$CaptureOnly,
    [switch]$ReloadPage,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference='Stop'
if(Test-Path $Output){throw "Refusing to overwrite $Output"}
$script:sequence=0
$script:events=[Collections.Generic.List[object]]::new()
function Connect-Cdp([string]$url){
    $socket=[Net.WebSockets.ClientWebSocket]::new()
    $null=$socket.ConnectAsync([Uri]$url,[Threading.CancellationToken]::None).GetAwaiter().GetResult()
    return $socket
}
function Invoke-Cdp($socket,[string]$method,$parameters=@{}){
    $script:sequence++;$id=$script:sequence
    $bytes=[Text.Encoding]::UTF8.GetBytes((@{id=$id;method=$method;params=$parameters}|ConvertTo-Json -Depth 40 -Compress))
    $null=$socket.SendAsync([ArraySegment[byte]]::new($bytes),[Net.WebSockets.WebSocketMessageType]::Text,$true,[Threading.CancellationToken]::None).GetAwaiter().GetResult()
    $timeout=[Threading.CancellationTokenSource]::new(30000)
    try{
        while($true){
            $stream=[IO.MemoryStream]::new()
            do{
                $buffer=[byte[]]::new(1048576)
                $received=$socket.ReceiveAsync([ArraySegment[byte]]::new($buffer),$timeout.Token).GetAwaiter().GetResult()
                if($received.MessageType -eq [Net.WebSockets.WebSocketMessageType]::Close){throw 'CDP closed'}
                $stream.Write($buffer,0,$received.Count)
            }while(-not $received.EndOfMessage)
            $message=[Text.Encoding]::UTF8.GetString($stream.ToArray())|ConvertFrom-Json
            $stream.Dispose()
            if($message.id -eq $id){
                if($message.error){throw ($message.error|ConvertTo-Json -Compress)}
                return $message.result
            }
            if($message.method -in @('Runtime.exceptionThrown','Log.entryAdded')){$script:events.Add($message)}
        }
    }finally{$timeout.Dispose()}
}
function Evaluate([string]$expression){
    $result=Invoke-Cdp $page 'Runtime.evaluate' @{expression=$expression;returnByValue=$true;awaitPromise=$true}
    if($result.exceptionDetails){throw ($result.exceptionDetails|ConvertTo-Json -Depth 20)}
    return $result.result.value
}
function Save-ViewerShots {
    $null=New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Output)
    $shot=Invoke-Cdp $page 'Page.captureScreenshot' @{format='png'}
    [IO.File]::WriteAllBytes([IO.Path]::ChangeExtension($Output,'.png'),[Convert]::FromBase64String($shot.data))
    $null=Evaluate 'document.querySelector("#overview").click()'
    Start-Sleep -Milliseconds 800
    $overview=Invoke-Cdp $page 'Page.captureScreenshot' @{format='png'}
    [IO.File]::WriteAllBytes([IO.Path]::ChangeExtension($Output,'.overview.png'),[Convert]::FromBase64String($overview.data))
    $null=Evaluate 'document.querySelector("#follow").click()'
}
$version=Invoke-RestMethod "http://127.0.0.1:$DebugPort/json/version"
$browser=Connect-Cdp $version.webSocketDebuggerUrl
$system=Invoke-Cdp $browser 'SystemInfo.getInfo'
$targets=Invoke-RestMethod "http://127.0.0.1:$DebugPort/json/list"
$target=$targets|Where-Object {$_.type -eq 'page' -and $_.url -like 'http://localhost:8080/native-view.html*'}|Select-Object -First 1
if(-not $target){
    $created=Invoke-Cdp $browser 'Target.createTarget' @{url='http://localhost:8080/native-view.html?fps=60'}
    Start-Sleep -Milliseconds 500
    $targets=Invoke-RestMethod "http://127.0.0.1:$DebugPort/json/list"
    $target=$targets|Where-Object {$_.id -eq $created.targetId}|Select-Object -First 1
}
if(-not $target){throw 'Could not open the isolated native viewer tab'}
$page=Connect-Cdp $target.webSocketDebuggerUrl
$null=Invoke-Cdp $page 'Runtime.enable'
$null=Invoke-Cdp $page 'Log.enable'
if($ReloadPage){
    $null=Invoke-Cdp $page 'Page.reload' @{ignoreCache=$true}
    Start-Sleep -Seconds 2
}
$null=Invoke-Cdp $page 'Page.bringToFront'
if($CaptureOnly){
    Save-ViewerShots
    $capture=@{kind='visual screenshot only, not performance acceptance';metrics=(Evaluate 'window.flybrainAcceptance()')}
    [IO.File]::WriteAllText($Output,($capture|ConvertTo-Json -Depth 40),[Text.UTF8Encoding]::new($false))
    $page.Dispose();$browser.Dispose();return
}
$deadline=[DateTime]::UtcNow.AddSeconds(120)
do{
    Start-Sleep -Seconds 2
    $ready=Evaluate 'Boolean(window.__flybrainViewerMetrics?.connected&&window.__flybrainViewerMetrics.latestSnapshot?.time_seconds>2&&window.__flybrainViewerMetrics.mainRenderFrames>60)'
}while(-not $ready -and [DateTime]::UtcNow -lt $deadline)
if(-not $ready){throw 'Native viewer did not become ready'}
Start-Sleep -Seconds 5
$setup=@'
(()=>{
 const m=window.__flybrainViewerMetrics;
 m.frameIntervalsMs.length=0;m.mainRenderDurationsMs.length=0;m.visibilityChanges.length=0;m.maxPoseAgeMs=0;
 const start=performance.now();
 const capture=()=>({wallMs:performance.now(),renderFrames:m.mainRenderFrames,poseFrames:m.freshPoseFrames,
   retinaFrames:m.retinaFrames,simSeconds:m.latestSnapshot.time_seconds,epoch:m.epoch,
   connected:m.connected,visibility:document.visibilityState,focused:document.hasFocus(),
   poseBufferFrames:m.poseBufferFrames,poseAgeMs:performance.now()-m.lastFreshPoseAt});
 const initial=capture();const samples=[];
 const record=()=>samples.push(capture());
 const interval=setInterval(record,1000);
 window.nativeAcceptanceRun={start,initial,samples,done:false};
 setTimeout(()=>{
   clearInterval(interval);record();
   const final=capture(),wall=(final.wallMs-initial.wallMs)/1000;
   const sorted=m.frameIntervalsMs.slice().sort((a,b)=>a-b);
   const result={wallSeconds:wall,renderFps:(final.renderFrames-initial.renderFrames)/wall,
     freshPoseHz:(final.poseFrames-initial.poseFrames)/wall,retinaHz:(final.retinaFrames-initial.retinaFrames)/wall,
     realtimeFactor:(final.simSeconds-initial.simSeconds)/wall,p95FrameMs:sorted[Math.floor(sorted.length*.95)],
     initial,final,framesMeasured:sorted.length,metrics:window.flybrainAcceptance()};
   const failures=[];
   if(result.renderFps<55)failures.push('render FPS below 55');
   if(result.p95FrameMs>25)failures.push('p95 frame interval above 25 ms');
   if(result.freshPoseHz<25||result.freshPoseHz>30.05)failures.push('fresh pose rate outside 25-30 Hz');
   if(result.retinaHz<10||result.retinaHz>15.05)failures.push('retina rate outside 10-15 Hz');
   if(result.realtimeFactor<1)failures.push('native simulation below realtime');
   if(samples.some(s=>!s.connected||s.visibility!=='visible'||s.epoch!==initial.epoch))failures.push('disconnected, hidden, or reset during measurement');
   if(m.visibilityChanges.some(s=>s.state!=='visible'))failures.push('viewer became hidden');
   if(!result.metrics.shadowEnabled)failures.push('shadows disabled');
   // The existing scene protocol calls the device name "backend".
   if(result.metrics.sceneBrain?.neurons!==166700||!/RTX 5080/i.test(result.metrics.sceneBrain?.backend??''))failures.push('wrong CNS/device');
   if(!/RTX 5080/i.test(result.metrics.glRenderer))failures.push('not RTX 5080 WebGL renderer');
   if(m.lastError)failures.push(m.lastError);
   if(m.maxPoseAgeMs>250)failures.push('stale pose over 250 ms');
   window.nativeAcceptanceRun.result={...result,failures,passed:!failures.length,formalDuration:wall>=600};
   window.nativeAcceptanceRun.done=true;
 },DURATION_MS);
 return initial;
})()
'@
$initial=Evaluate ($setup.Replace('DURATION_MS',[string]($DurationSeconds*1000)))
Write-Output ($initial|ConvertTo-Json -Compress)
do{
    Start-Sleep -Seconds 5
    $state=Evaluate '({done:window.nativeAcceptanceRun.done,last:window.nativeAcceptanceRun.samples.at(-1)})'
    Write-Output ($state|ConvertTo-Json -Compress)
}while(-not $state.done)
$result=Evaluate '({result:window.nativeAcceptanceRun.result,samples:window.nativeAcceptanceRun.samples})'
$report=@{recorded_utc=[DateTime]::UtcNow.ToString('o');browser=$version.Browser;system_gpu=$system.gpu;
    events=$script:events.ToArray();measurement=$result;duration_requested_seconds=$DurationSeconds}
$null=New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Output)
[IO.File]::WriteAllText($Output,($report|ConvertTo-Json -Depth 50),[Text.UTF8Encoding]::new($false))
Save-ViewerShots
$page.Dispose();$browser.Dispose()
Write-Output ($result.result|ConvertTo-Json -Depth 30)
if(-not $result.result.passed){throw 'Native viewer acceptance failed; report saved'}
