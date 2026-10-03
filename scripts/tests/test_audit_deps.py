import json
import re

import pytest


@pytest.fixture
def audit(script):
    return script("audit-deps")


PAGE = """\
<h3>Reporting vulnerabilities</h3>
<p>Any CVE identifier, if available.</p>
<h2>FFmpeg git master</h2>
Fixes following vulnerabilities:
CVE-2026-1, 0469d68, BIGSLEEP-1
CVE-2026-2, d311382
<h2>FFmpeg 9.0</h2>
<h3>9.0.3</h3>
Fixes following vulnerabilities:
CVE-2026-3, c23d4da, pr/1
CVE-2026-3, 5806e8b, pr/1
<h3>9.0</h3>
Fixes following vulnerabilities:
CVE-2026-4, 2f60af4
<h2>FFmpeg 8.1</h2>
<h3>8.1.2</h3>
CVE-2026-5, a991b3e
<h2 id="10.0">FFmpeg 10.0</h2>
<h3 id="10.0">10.0</h3>
CVE-2026-6, abc
"""


def test_security_fixes_by_release(audit):
    assert audit.security_fixes(PAGE) == {
        "master": ["CVE-2026-1", "CVE-2026-2"],
        "9.0.3": ["CVE-2026-3"],
        "9.0": ["CVE-2026-4"],
        "8.1.2": ["CVE-2026-5"],
        "10.0": ["CVE-2026-6"],
    }


def test_a_newer_release_of_the_pinned_branch_fails(audit):
    fixes = audit.security_fixes(PAGE)
    failures, notes = audit.ffmpeg_findings("9.0.2", fixes)
    assert failures == ["FFmpeg 9.0.3 fixes CVE-2026-3; the pin is 9.0.2"]
    assert notes == [
        "2 fixed in git master, not yet in a release",
        "FFmpeg 10.0 (a newer branch) fixes 1",
    ]
    failures, _ = audit.ffmpeg_findings("9.0.3", fixes)
    assert failures == []


def test_the_ffmpeg_check_reads_the_pin_and_page(audit):
    lines = []
    assert not audit.check_ffmpeg(lambda url: PAGE.replace("9.0.3", "9.0.9"), out=lines.append)
    assert any(line.startswith("FAIL  FFmpeg 9.0.9") for line in lines)

    lines = []
    assert audit.check_ffmpeg(lambda url: "<h2>FFmpeg 1.0</h2>\n<h3>1.0</h3>\n", out=lines.append)
    assert lines[-1].startswith("ok    FFmpeg")


def test_a_changed_or_unreachable_page_fails(audit):
    lines = []
    assert not audit.check_ffmpeg(lambda url: "<html>moved</html>", out=lines.append)
    assert "has the page changed?" in lines[0]

    def offline(url):
        raise OSError("offline")

    assert not audit.check_ffmpeg(offline, out=lines.append)


def npm_report(*advisories):
    return json.dumps(
        {
            "vulnerabilities": {
                "@sveltejs/kit": {"via": ["cookie"]},
                **{
                    package: {
                        "via": [
                            {
                                "source": 1,
                                "url": f"https://github.com/advisories/{advisory}",
                                "severity": "low",
                                "title": "t",
                            }
                        ]
                    }
                    for package, advisory in advisories
                },
            }
        }
    )


def test_npm_advisories_fail_unless_allowed_with_a_reason(audit):
    allowed = next(iter(audit.NPM_IGNORED))
    lines = []
    assert audit.check_npm(npm_report(("cookie", allowed)), out=lines.append)
    assert lines[0].startswith(f"allowed  {allowed} cookie (low): t:")

    lines = []
    assert not audit.check_npm(
        npm_report(("cookie", allowed), ("vite", "GHSA-new")), out=lines.append
    )
    assert "FAIL     GHSA-new vite (low): t" in lines

    lines = []
    assert audit.check_npm(json.dumps({"vulnerabilities": {}}), out=lines.append)
    assert any("no longer reported" in line for line in lines)
    assert lines[-1] == "ok       npm audit: no advisories"

    assert not audit.check_npm("npm error", out=lines.append)


def test_every_allowed_advisory_has_a_reason(audit):
    for advisory, reason in audit.NPM_IGNORED.items():
        assert advisory.startswith("GHSA-") and reason.strip()


def test_the_real_ffmpeg_pin_is_read(audit):
    assert re.fullmatch(r"\d+(\.\d+)+", audit.ffmpeg_pin())


def workflows(audit):
    return sorted((audit.REPO_ROOT / ".github" / "workflows").glob("*.yml"))


def test_every_workflow_action_is_pinned_by_commit(audit):
    for workflow in workflows(audit):
        for line in workflow.read_text().splitlines():
            # A workflow in this repository (./.github/...) needs no pin.
            if "uses:" in line and "uses: ./" not in line:
                assert re.search(r"uses: [\w./-]+@[0-9a-f]{40} # v[\d.]+$", line), (
                    workflow.name,
                    line,
                )


def test_the_audit_workflow_runs_the_scripts_and_cis_cargo_deny(audit):
    folder = audit.REPO_ROOT / ".github" / "workflows"
    text = (folder / "audit.yml").read_text()
    assert "scripts/audit-deps.py" in text
    assert "scripts/check-pins.py" in text
    assert "schedule:" in text
    pattern = r"cargo install cargo-deny --version ([\d.]+)"
    ci = re.search(pattern, (folder / "ci.yml").read_text()).group(1)
    assert re.findall(pattern, text) == [ci]
    assert f"cargo-deny-${{{{ runner.os }}}}-{ci}" in text
