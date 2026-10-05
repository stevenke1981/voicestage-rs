param([ValidateSet('release','balanced','compact','fast','all')][string]$Profile='release')
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$oldFlags=$env:CARGO_ENCODED_RUSTFLAGS
$oldLocation=Get-Location
try {
    # Remap compiler diagnostics/panic source paths; never ship the builder's
    # home directory. Encoded flags preserve paths containing spaces.
    $homePath=[Environment]::GetFolderPath('UserProfile')
    $flags=@("--remap-path-prefix=$homePath=/builder", "--remap-path-prefix=$($homePath.Replace('\','/'))=/builder", "--remap-path-prefix=$repo=/voicestage")
    $env:CARGO_ENCODED_RUSTFLAGS=$flags -join [char]31
    Set-Location $repo
    $profiles=if($Profile -eq 'all'){@('release','balanced','compact')}else{@($Profile)}
    foreach($current in $profiles) {
        cargo build --locked --profile $current -j 4
        if($LASTEXITCODE -ne 0){throw "Build failed: $current"}
        Get-Item "target\$current\voicestage.exe" | Select-Object Name,Length
    }
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS=$oldFlags
    Set-Location $oldLocation
}
