param(
    [Parameter(Mandatory)][string]$Root,
    [string]$Previous
)

$ErrorActionPreference = 'Stop'
$directory = Get-Item -LiteralPath $Root -Force
if (!$directory.PSIsContainer) { throw "Expected a directory: $Root" }
$prefix = $directory.FullName.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
$previousByPath = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
if ($Previous) {
    foreach ($old in Import-Csv -LiteralPath $Previous) {
        if ($old.Change -ne 'Removed') { $previousByPath.Add($old.Path, $old) }
    }
}

function ReadInventory($folder) {
    foreach ($entry in Get-ChildItem -LiteralPath $folder -Force | Sort-Object Name) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Choose the physical source directory for reparse point: $($entry.FullName)"
        }
        if ($entry.PSIsContainer) {
            ReadInventory $entry.FullName
            continue
        }
        $relative = $entry.FullName.Substring($prefix.Length).Replace('\', '/')
        $stream = [IO.File]::OpenRead($entry.FullName)
        try { $digest = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '') }
        finally { $stream.Dispose() }
        $old = $null
        $change = if (!$Previous) { 'Present' }
            elseif (!$previousByPath.TryGetValue($relative, [ref]$old)) { 'Added' }
            elseif ($old.SHA256 -ne $digest) { 'Changed' }
            else { 'Unchanged' }
        [void]$previousByPath.Remove($relative)
        [pscustomobject]@{
            Path = $relative
            Kind = if ($entry.Name.StartsWith('._')) { 'AppleDouble' } else { $entry.Extension.ToLowerInvariant() }
            Bytes = $entry.Length
            SHA256 = $digest
            Change = $change
        }
    }
}

$sha = [Security.Cryptography.SHA256]::Create()
try {
    & {
        ReadInventory $directory.FullName
        foreach ($old in $previousByPath.Values | Sort-Object Path) {
            $old.Change = 'Removed'
            $old
        }
    } | ConvertTo-Csv -NoTypeInformation
} finally { $sha.Dispose() }
