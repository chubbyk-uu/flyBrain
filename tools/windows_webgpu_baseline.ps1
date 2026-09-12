param(
    [ValidateSet('qa', 'perf', 'reset')][string]$Mode = 'qa',
    [ValidateSet('rendered', 'cns', 'world')][string]$Scenario = 'rendered',
    [switch]$NoVision,
    [switch]$Timestamps,
    [int]$Windows = 10000,
    [int]$DebugPort = 9222,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
if (Test-Path $Output) { throw "Refusing to overwrite $Output" }
$script:sequence = 0
$script:events = [Collections.Generic.List[object]]::new()
function Connect-Cdp([string]$url) {
    $socket = [Net.WebSockets.ClientWebSocket]::new()
    $null = $socket.ConnectAsync([Uri]$url, [Threading.CancellationToken]::None).GetAwaiter().GetResult()
    return $socket
}
function Invoke-Cdp($socket, [string]$method, $parameters = @{}) {
    $script:sequence++
    $id = $script:sequence
    $bytes = [Text.Encoding]::UTF8.GetBytes((@{id=$id;method=$method;params=$parameters} | ConvertTo-Json -Depth 40 -Compress))
    $null = $socket.SendAsync([ArraySegment[byte]]::new($bytes), [Net.WebSockets.WebSocketMessageType]::Text,
        $true, [Threading.CancellationToken]::None).GetAwaiter().GetResult()
    while ($true) {
        $stream = [IO.MemoryStream]::new()
        do {
            $buffer = [byte[]]::new(1048576)
            $received = $socket.ReceiveAsync([ArraySegment[byte]]::new($buffer),
                [Threading.CancellationToken]::None).GetAwaiter().GetResult()
            if ($received.MessageType -eq [Net.WebSockets.WebSocketMessageType]::Close) { throw 'CDP closed' }
            $stream.Write($buffer, 0, $received.Count)
        } while (-not $received.EndOfMessage)
        $message = [Text.Encoding]::UTF8.GetString($stream.ToArray()) | ConvertFrom-Json
        $stream.Dispose()
        if ($message.id -eq $id) {
            if ($message.error) { throw ($message.error | ConvertTo-Json -Compress) }
            return $message.result
        }
        if ($message.method -in @('Runtime.exceptionThrown', 'Log.entryAdded')) { $script:events.Add($message) }
    }
}
function Evaluate([string]$expression) {
    $result = Invoke-Cdp $page 'Runtime.evaluate' @{expression=$expression;returnByValue=$true;awaitPromise=$true}
    if ($result.exceptionDetails) { throw ($result.exceptionDetails | ConvertTo-Json -Depth 20) }
    return $result.result.value
}
$version = Invoke-RestMethod "http://127.0.0.1:$DebugPort/json/version"
$browser = Connect-Cdp $version.webSocketDebuggerUrl
$system = Invoke-Cdp $browser 'SystemInfo.getInfo'
$targets = Invoke-RestMethod "http://127.0.0.1:$DebugPort/json/list"
$target = $targets | Where-Object { $_.type -eq 'page' -and $_.url -like 'http://localhost:8080/*' } | Select-Object -First 1
if (-not $target) { throw 'No isolated FlyBrain page found' }
$page = Connect-Cdp $target.webSocketDebuggerUrl
$null = Invoke-Cdp $page 'Runtime.enable'
$null = Invoke-Cdp $page 'Log.enable'
$path = if ($Mode -eq 'qa') { 'neural-test.html' } else { 'perf-test.html' }
$null = Invoke-Cdp $page 'Page.navigate' @{url="http://localhost:8080/$path"}
Start-Sleep -Seconds 2
$gpu = Evaluate @'
(async()=>{const a=await navigator.gpu.requestAdapter({powerPreference:'high-performance'});
if(!a)throw Error('No WebGPU adapter');const i=a.info;return {userAgent:navigator.userAgent,
secureContext:isSecureContext,crossOriginIsolated,adapter:{vendor:i.vendor,architecture:i.architecture,
device:i.device,description:i.description,isFallbackAdapter:i.isFallbackAdapter},
limits:{maxBufferSize:a.limits.maxBufferSize,maxStorageBufferBindingSize:a.limits.maxStorageBufferBindingSize}}})()
'@
Write-Output ($gpu | ConvertTo-Json -Depth 8 -Compress)
if ($Mode -eq 'reset') {
    $result = Evaluate @'
new Promise((resolve,reject)=>{const w=new Worker('./simulation-worker.js',{type:'module'});
let initial,progress,requested=false,scene;const timer=setTimeout(()=>{w.terminate();reject(Error('Reset timeout'))},120000);
w.onmessage=({data:d})=>{if(d.type==='scene')scene=d.scene;if(d.type==='error'){clearTimeout(timer);w.terminate();reject(Error(d.message))}
if(d.type!=='frame')return;const s=d.snapshot;if(!initial){initial=s;return}
if(!requested&&s.time_seconds>=0.2){progress=s;requested=true;w.postMessage({type:'pause',paused:true});w.postMessage({type:'command',command:'reset'});return}
if(requested&&s.time_seconds===0){const same=JSON.stringify([s.qpos,s.qvel,s.total_spikes])===JSON.stringify([initial.qpos,initial.qvel,initial.total_spikes]);
clearTimeout(timer);w.terminate();resolve({status:same&&progress.total_spikes>0?'PASS':'FAIL',sceneBrain:scene.brain,
initial:{time:initial.time_seconds,spikes:initial.total_spikes},progress:{time:progress.time_seconds,spikes:progress.total_spikes},
reset:{time:s.time_seconds,spikes:s.total_spikes},exactInitialState:same})}};
w.postMessage({type:'start',brain:true})})
'@
    $report = @{mode=$Mode;browser=$version.Browser;gpu=$gpu;result=$result}
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Output)
    [IO.File]::WriteAllText($Output, ($report | ConvertTo-Json -Depth 30), [Text.UTF8Encoding]::new($false))
    $page.Dispose(); $browser.Dispose()
    Write-Output ($report | ConvertTo-Json -Depth 30)
    if ($result.status -ne 'PASS') { throw 'Reset check failed' }
    exit
}
if ($Mode -eq 'qa') { $null = Evaluate "document.querySelector('#run').click()" }
else {
    $visionValue = if ($NoVision) { 'false' } else { 'true' }
    $timestampValue = if ($Timestamps) { 'true' } else { 'false' }
    $button = "#$Scenario"
    $null = Evaluate "document.querySelector('#windows').value='$Windows';document.querySelector('#baseline').checked=false;document.querySelector('#timestamps').checked=$timestampValue;document.querySelector('#vision').checked=$visionValue;document.querySelector('$button').click()"
}
$deadline = [DateTime]::UtcNow.AddMinutes(15)
do {
    Start-Sleep -Seconds 5
    $state = Evaluate "({status:document.querySelector('#status')?.textContent,results:document.querySelector('#results').textContent,report:document.querySelector('#report')?.textContent})"
    Write-Output $(if ($Mode -eq 'qa') { $state.results } else { $state.status })
    if ($Mode -eq 'qa' -and $state.results -match '"status"\s*:\s*"(PASS|FAIL)"') { break }
    if ($Mode -eq 'perf' -and ($state.status -eq 'Complete' -or $state.results)) { break }
} while ([DateTime]::UtcNow -lt $deadline)
if ($Mode -eq 'qa') { $result = $state.results | ConvertFrom-Json }
elseif ($state.report) { $result = $state.report | ConvertFrom-Json }
else { throw "Benchmark failed or timed out: $($state.results)" }
$report = @{recorded_utc=[DateTime]::UtcNow.ToString('o');mode=$Mode;browser=$version.Browser;
    gpu=$gpu;system_gpu=$system.gpu;events=$script:events.ToArray();result=$result}
$parent = Split-Path -Parent $Output
$null = New-Item -ItemType Directory -Force -Path $parent
[IO.File]::WriteAllText($Output, ($report | ConvertTo-Json -Depth 100), [Text.UTF8Encoding]::new($false))
$shot = Invoke-Cdp $page 'Page.captureScreenshot' @{format='png'}
[IO.File]::WriteAllBytes([IO.Path]::ChangeExtension($Output, '.png'), [Convert]::FromBase64String($shot.data))
$page.Dispose()
$browser.Dispose()
Write-Output "Saved $Output"
if ($Mode -eq 'qa' -and $result.status -ne 'PASS') { throw 'WebGPU QA failed; report saved' }
