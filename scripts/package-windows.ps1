<#
.SYNOPSIS
Build and package ESP32 SuperFlash for Windows x64.

.DESCRIPTION
Produces a portable ZIP and, when Inno Setup 6.5+ is available, a setup EXE in dist/.
By default it prepares the bundled tools with uv and compiles the release binary with cargo.
Use -SkipTools / -SkipBuild when the tools and binary are already present.
The Inno Setup compiler can be supplied with -ISCC or the INNO_ISCC environment variable.
#>
param(
    [switch]$SkipTools,
    [switch]$SkipBuild,
    [string]$ISCC
)

$ErrorActionPreference = "Stop"

function Add-SuperFlashIcon {
    param([string]$PngPath, [string]$IconPath, [string]$ExePath)

    Add-Type -AssemblyName System.Drawing
    if (-not ('SuperFlashIconResources' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;

public static class SuperFlashIconResources
{
    private const int RtIcon = 3;
    private const int RtGroupIcon = 14;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    private static extern IntPtr BeginUpdateResourceW(string file, bool deleteExisting);

    [DllImport("kernel32.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool UpdateResourceW(IntPtr update, IntPtr type, IntPtr id,
        ushort language, byte[] data, uint length);

    [DllImport("kernel32.dll", ExactSpelling = true, SetLastError = true)]
    private static extern bool EndUpdateResourceW(IntPtr update, bool discard);

    private static void WriteHeader(BinaryWriter writer, int count)
    {
        writer.Write((ushort)0);
        writer.Write((ushort)1);
        writer.Write((ushort)count);
    }

    private static void WriteEntry(BinaryWriter writer, int size, int length)
    {
        byte dimension = size == 256 ? (byte)0 : (byte)size;
        writer.Write(dimension);
        writer.Write(dimension);
        writer.Write((ushort)0); // Color count and reserved byte.
        writer.Write((ushort)1);
        writer.Write((ushort)32);
        writer.Write(length);
    }

    private static void PutResource(IntPtr update, int type, int id, byte[] data)
    {
        if (!UpdateResourceW(update, new IntPtr(type), new IntPtr(id), 0, data, (uint)data.Length))
            throw new Win32Exception(Marshal.GetLastWin32Error());
    }

    public static void Write(string iconPath, string exePath, int[] sizes, byte[][] images)
    {
        using (var icon = new BinaryWriter(File.Create(iconPath)))
        {
            WriteHeader(icon, sizes.Length);
            int offset = 6 + 16 * sizes.Length;
            for (int i = 0; i < sizes.Length; i++)
            {
                WriteEntry(icon, sizes[i], images[i].Length);
                icon.Write(offset);
                offset += images[i].Length;
            }
            foreach (byte[] image in images)
                icon.Write(image);
        }

        IntPtr update = BeginUpdateResourceW(exePath, false);
        if (update == IntPtr.Zero)
            throw new Win32Exception(Marshal.GetLastWin32Error());

        bool ready = false;
        try
        {
            using (var group = new MemoryStream())
            using (var writer = new BinaryWriter(group))
            {
                WriteHeader(writer, sizes.Length);
                for (int i = 0; i < sizes.Length; i++)
                {
                    int resourceId = i + 1;
                    PutResource(update, RtIcon, resourceId, images[i]);
                    WriteEntry(writer, sizes[i], images[i].Length);
                    writer.Write((ushort)resourceId);
                }
                PutResource(update, RtGroupIcon, 1, group.ToArray());
            }
            ready = true;
        }
        finally
        {
            if (!EndUpdateResourceW(update, !ready) && ready)
                throw new Win32Exception(Marshal.GetLastWin32Error());
        }
    }
}
'@
    }

    [int[]]$sizes = 16, 24, 32, 48, 64, 128, 256
    [byte[][]]$images = @()
    $original = [System.Drawing.Image]::FromFile($PngPath)
    try {
        foreach ($size in $sizes) {
            $bitmap = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
            $stream = [System.IO.MemoryStream]::new()
            try {
                $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
                try {
                    $graphics.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
                    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                    $graphics.DrawImage($original, 0, 0, $size, $size)
                } finally {
                    $graphics.Dispose()
                }
                $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
                $images += ,($stream.ToArray())
            } finally {
                $stream.Dispose()
                $bitmap.Dispose()
            }
        }
    } finally {
        $original.Dispose()
    }
    [SuperFlashIconResources]::Write($IconPath, $ExePath, $sizes, $images)
}

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $Root
try {
    $Target = "x86_64-pc-windows-msvc"
    $Platform = "windows-x86_64"
    if (-not $SkipTools) {
        uv run --python 3.12 --with-requirements scripts/build-requirements.txt python scripts/fetch-tools.py
        if ($LASTEXITCODE -ne 0) { throw "Failed to prepare bundled tools" }
    }
    $required = @(
        "tools/$Platform/versions.txt",
        "tools/$Platform/esptool.exe",
        "tools/$Platform/python/python.exe",
        "tools/shared/versions.txt",
        "tools/shared/idf-image.pyz"
    )
    foreach ($file in $required) {
        if (-not (Test-Path $file)) { throw "Missing $file; run the script without -SkipTools" }
    }

    $Exe = "target/$Target/release/esp32-superflash.exe"
    if (-not $SkipBuild) {
        cargo build --locked --release --target $Target --target-dir target
        if ($LASTEXITCODE -ne 0) { throw "Rust build failed" }
    }
    if (-not (Test-Path $Exe)) { throw "Missing $Exe; run the script without -SkipBuild" }

    $Version = (Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    $Name = "ESP32-SuperFlash-$Version-$Platform"
    $DistDir = Join-Path $Root "dist"
    New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

    $TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString("N"))
    $Stage = Join-Path $TempRoot $Name
    try {
        New-Item -ItemType Directory -Force -Path "$Stage/tools" | Out-Null
        $StagedExe = Join-Path $Stage "ESP32-SuperFlash.exe"
        Copy-Item $Exe $StagedExe -Force
        $IconFile = Join-Path $TempRoot "SuperFlash.ico"
        Add-SuperFlashIcon -PngPath (Join-Path $Root "assets/icon.png") -IconPath $IconFile -ExePath $StagedExe
        Copy-Item -Recurse -Force "tools/$Platform", "tools/shared" "$Stage/tools/"
        Copy-Item "README.md", "README.zh-CN.md", "LICENSE" "$Stage/" -Force
        New-Item -ItemType Directory -Path "$Stage/assets" | Out-Null
        Copy-Item -Recurse "assets/screenshots" "$Stage/assets/"
        Copy-Item "tools/README.md" "$Stage/tools/" -Force
        Get-ChildItem -Path $Stage -Recurse -Force -Filter ".DS_Store" | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force }

        $Zip = Join-Path $DistDir "$Name.zip"
        Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal -Force
        Write-Output $Zip

        $compiler = $ISCC
        if (-not $compiler) { $compiler = $env:INNO_ISCC }
        if (-not $compiler) { $compiler = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source }
        if (-not $compiler) {
            $candidates = @(
                (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe"),
                (Join-Path "${env:ProgramFiles(x86)}" "Inno Setup 6\ISCC.exe"),
                (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe")
            )
            $compiler = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
        }
        if (-not $compiler) {
            Write-Warning "Inno Setup (ISCC.exe) not found; only the portable ZIP was produced. Install Inno Setup 6.5+ or pass -ISCC."
        } else {
            $SetupBase = "$Name-setup"
            & $compiler "/DStage=$Stage" "/DRoot=$Root" "/DVersion=$Version" "/DIconFile=$IconFile" "/O$DistDir" "/F$SetupBase" (Join-Path $PSScriptRoot "installer.iss")
            if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed" }
            Write-Output (Join-Path $DistDir "$SetupBase.exe")
        }
    } finally {
        if (Test-Path $TempRoot) { Remove-Item -Recurse -Force $TempRoot }
    }
} finally {
    Pop-Location
}
