# Installs Estigia on Windows.
#
#   irm https://raw.githubusercontent.com/asanabrial/estigia/main/scripts/install.ps1 | iex
#
# Downloads the release archive for this machine, checks it against the
# published SHA-256 sums, and puts the binary on the user's PATH. Nothing is
# installed if the checksum does not match.
#
# This script and install.sh share no logic. They only download and verify,
# which is the whole reason each fits in a page.

$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 is what Windows ships, and it does not negotiate TLS
# 1.2 unless told to. GitHub refuses anything older, so without this the very
# first download fails on a default machine.
try {
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch {
    # PowerShell 7 manages this itself and may not expose the property.
}

$repo = 'asanabrial/estigia'
$installDir = if ($env:ESTIGIA_INSTALL_DIR) { $env:ESTIGIA_INSTALL_DIR } else { "$env:LOCALAPPDATA\estigia\bin" }
$version = if ($env:ESTIGIA_VERSION) { $env:ESTIGIA_VERSION } else { 'latest' }

# Named in every refusal that has no prebuilt binary to offer. Six build targets
# have to exist before anyone can install; until they do, a source build is the
# continuation that always works, and a refusal that does not name it is a dead
# end.
$sourceBuild = "cargo install --git https://github.com/$repo"

function Fail($message) {
    Write-Error "error: $message"
    exit 1
}

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { Fail "no prebuilt Estigia for $($env:PROCESSOR_ARCHITECTURE); build from source with '$sourceBuild'" }
}
$target = "$arch-pc-windows-msvc"

if ($version -eq 'latest') {
    # The redirect from /releases/latest names the tag, which avoids depending
    # on the API and its rate limit.
    #
    # Read through .NET rather than Invoke-WebRequest: the switches for holding
    # a redirect differ between Windows PowerShell 5.1 and PowerShell 7, and
    # this has to work on the one already installed.
    $location = $null
    try {
        $request = [System.Net.WebRequest]::Create("https://github.com/$repo/releases/latest")
        $request.Method = 'HEAD'
        $request.AllowAutoRedirect = $false
        $response = $request.GetResponse()
        $location = $response.Headers['Location']
        $response.Close()
    } catch [System.Net.WebException] {
        # A 404 means there is nothing published yet, which is a different
        # problem from a network that is down, and sends the reader somewhere
        # else entirely.
        $status = $_.Exception.Response.StatusCode.value__
        if ($status -eq 404) {
            Fail "$repo has no published releases yet; build from source with '$sourceBuild'"
        }
        Fail "could not reach GitHub to find the latest version ($status); set ESTIGIA_VERSION"
    } catch {
        Fail "could not reach GitHub to find the latest version; set ESTIGIA_VERSION"
    }
    if (-not $location) { Fail "could not work out the latest version; set ESTIGIA_VERSION" }
    $version = ($location -split '/tag/')[-1]
}

$package = "estigia-$version-$target"
$archive = "$package.zip"
# Overridable so an internal mirror can serve the same layout, and so the
# verification path can be exercised without publishing anything.
$base = if ($env:ESTIGIA_BASE_URL) { $env:ESTIGIA_BASE_URL } else { "https://github.com/$repo/releases/download/$version" }

Write-Host "Estigia $version for $target"

$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("estigia-install-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $temp | Out-Null
try {
    Write-Host "  downloading"
    try {
        Invoke-WebRequest -Uri "$base/$archive" -OutFile "$temp\$archive"
    } catch {
        Fail "no release archive at $base/$archive; build from source with '$sourceBuild'"
    }
    # One sum per archive, which is what the release workflow publishes — there
    # has never been an aggregate listing. Both installers asked for
    # `SHA256SUMS` and refused when it was missing: fail-closed, correct, and
    # about a release that was complete.
    try {
        Invoke-WebRequest -Uri "$base/$archive.sha256" -OutFile "$temp\$archive.sha256"
    } catch {
        Fail "no checksum published for $archive; refusing to install unverified"
    }

    Write-Host "  verifying"
    $expected = (-split (Get-Content "$temp\$archive.sha256" -Raw))[0]
    if (-not $expected) { Fail "$archive.sha256 carries no checksum" }
    $actual = (Get-FileHash "$temp\$archive" -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected.ToLower()) {
        Fail "checksum mismatch: expected $expected, got $actual"
    }

    # The sum says the bytes are the ones published; it says nothing about who
    # published them, because whoever could replace the archive could replace
    # the sum beside it. The provenance answers that, signed by the workflow
    # run that built it with an identity nobody holds.
    #
    # Only when `gh` is here: an installer that refuses to work without the
    # GitHub CLI is one people remove rather than fix. When it IS here and says
    # the signature is bad, that is a hard stop.
    if (Get-Command gh -ErrorAction SilentlyContinue) {
        Write-Host "  checking provenance"
        gh attestation verify "$temp\$archive" --repo $repo *> $null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  provenance: signed by the $repo release workflow"
        } else {
            gh auth status *> $null
            if ($LASTEXITCODE -eq 0) {
                Fail "provenance check FAILED for $archive - the bytes match the published sum but nothing proves the $repo workflow built them. Refusing to install."
            } else {
                Write-Host "  provenance: not checked (gh is not logged in)"
            }
        }
    }

    Write-Host "  extracting"
    Expand-Archive -Path "$temp\$archive" -DestinationPath $temp -Force
    $candidate = "$temp\$package\estigia.exe"
    Write-Host "  recording candidate lifecycle"
    & $candidate '__record-install'
    if ($LASTEXITCODE -ne 0) {
        Fail "candidate lifecycle admission failed; refusing to replace the installed executable"
    }

    Write-Host "  installing to $installDir"
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item $candidate (Join-Path $installDir 'estigia.exe') -Force

    # Read the registry value rather than the API's, and write it back the same
    # way. Two reasons, and both are somebody else's PATH being damaged by an
    # installer they let in:
    #
    #  - `[Environment]::GetEnvironmentVariable(..., 'User')` hands back the
    #    value with `%VAR%` already expanded, and `SetEnvironmentVariable`
    #    writes REG_SZ. A user PATH holding a `%USERPROFILE%` entry — which is how
    #    several popular installers write theirs — would come back frozen to
    #    whatever it expanded to, and stop following the variable.
    #  - A fresh account has no user PATH at all, and joining onto nothing puts
    #    a **separator first**. An empty entry in PATH means the current directory:
    #    every command typed anywhere would look in that folder first.
    #
    # No broadcast, and none is needed: the message below already says to open a
    # new terminal, and a new one reads this fresh.
    $key = 'HKCU:\Environment'
    # `GetValue` with `DoNotExpandEnvironmentNames`, not `Get-ItemProperty`:
    # that one expands the value as it reads it, so preserving the *kind* while
    # writing back an expanded *value* leaves the entry an ExpandString with
    # nothing left to expand. Measured on a throwaway key: a `%USERPROFILE%` entry
    # came back as the literal profile path, which is the freezing this exists
    # to prevent, done more quietly.
    $reg = Get-Item -Path $key -ErrorAction SilentlyContinue
    $userPath = if ($reg) { [string]$reg.GetValue('Path', '', 'DoNotExpandEnvironmentNames') } else { '' }
    $kind = if ($reg -and $reg.GetValue('Path')) { $reg.GetValueKind('Path') } else { 'String' }
    if (($userPath -split ';') -notcontains $installDir) {
        $joined = if ([string]::IsNullOrEmpty($userPath)) { $installDir } else { "$userPath;$installDir" }
        Set-ItemProperty -Path $key -Name Path -Value $joined -Type $kind
        Write-Host ""
        Write-Host "Installed, and $installDir was added to your PATH."
        Write-Host "Open a new terminal, then run 'estigia setup --all'."
    } else {
        Write-Host ""
        Write-Host "Installed. Run 'estigia setup --all' to register it in your agents."
    }
} finally {
    Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
}
