# Écrit le résumé des tests Rust dans la page du job, à partir du rapport JUnit de nextest.
param(
    [string]$Report = "target/nextest/ci/junit.xml",
    [string]$Summary = $env:GITHUB_STEP_SUMMARY
)

$ErrorActionPreference = "Stop"
$invariant = [Globalization.CultureInfo]::InvariantCulture
$french = [Globalization.CultureInfo]::GetCultureInfo("fr-FR")

function Format-Number([double]$value, [string]$format) { $value.ToString($format, $french) }

if (-not (Test-Path $Report)) {
    "## Tests Rust`n`n⚠️ Rapport introuvable : les tests n'ont pas pu se lancer (voir le journal du job)." |
        Out-File -FilePath $Summary -Append -Encoding utf8
    exit 0
}

[xml]$xml = Get-Content -Path $Report -Raw -Encoding UTF8
$run = $xml.testsuites

$crates = [ordered]@{}
foreach ($suite in $run.SelectNodes("testsuite")) {
    $name = ($suite.name -split "::")[0]
    if (-not $crates.Contains($name)) { $crates[$name] = @{ Tests = 0; Failed = 0; Skipped = 0 } }
    $crates[$name].Tests += [int]$suite.tests
    $crates[$name].Failed += [int]$suite.failures + [int]$suite.errors
    $crates[$name].Skipped += [int]$suite.skipped
}

$total = [int]$run.tests
$failed = [int]$run.failures + [int]$run.errors
$skipped = [int]$run.skipped
$seconds = Format-Number ([double]::Parse($run.time, $invariant)) "N1"

$lines = @("## Tests Rust", "")
if ($failed -eq 0) {
    $lines += "✅ **$(Format-Number ($total - $skipped) 'N0') tests réussis** en $seconds s"
} else {
    $lines += "❌ **$(Format-Number $failed 'N0') échec(s)** sur $(Format-Number $total 'N0') tests, en $seconds s"
}
if ($skipped -gt 0) { $lines[-1] += " · $skipped ignoré(s)" }

$lines += "", "| Crate | Tests | Réussis | Échecs | Ignorés |", "|---|--:|--:|--:|--:|"
foreach ($name in $crates.Keys | Sort-Object) {
    $c = $crates[$name]
    $passed = $c.Tests - $c.Failed - $c.Skipped
    $mark = if ($c.Failed -gt 0) { "❌" } else { "✅" }
    $lines += "| $mark ``$name`` | $($c.Tests) | $passed | $($c.Failed) | $($c.Skipped) |"
}

$failures = $xml.SelectNodes("//testcase[failure or error]")
if ($failures.Count -gt 0) {
    $lines += "", "### Tests en échec", ""
    foreach ($case in $failures) { $lines += "- ``$($case.classname)`` · ``$($case.name)``" }
    $lines += "", "Le détail de chaque échec est dans le journal de l'étape « Tests »."
}

($lines -join "`n") + "`n" | Out-File -FilePath $Summary -Append -Encoding utf8
