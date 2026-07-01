function getInvoke() {
  const tauri = window.__TAURI__;
  if (tauri?.core?.invoke) {
    return tauri.core.invoke.bind(tauri.core);
  }
  return null;
}

async function invoke(cmd, args = {}) {
  const call = getInvoke();
  if (!call) {
    throw new Error(
      "Desktop bridge not ready. Quit and relaunch with: cd desktop/src-tauri && cargo tauri dev"
    );
  }
  return call(cmd, args);
}

function waitForTauri(maxMs = 5000) {
  return new Promise((resolve, reject) => {
    if (getInvoke()) {
      resolve();
      return;
    }
    const started = Date.now();
    const timer = setInterval(() => {
      if (getInvoke()) {
        clearInterval(timer);
        resolve();
      } else if (Date.now() - started > maxMs) {
        clearInterval(timer);
        reject(new Error("Desktop bridge not ready after launch."));
      }
    }, 50);
  });
}

const SCREENS = {
  profiles: {
    eyebrow: "Step 1 of 3",
    title: "Delivery profiles",
    hint: "Choose a built-in spec or create a custom profile for your client.",
  },
  check: {
    eyebrow: "Step 2 of 3",
    title: "Check your export",
    hint: "Choose a delivery profile and point at the folder or file to validate.",
  },
  results: {
    eyebrow: "Step 3 of 3",
    title: "Review results",
    hint: "Start with the batch verdict, then drill into files that need attention.",
  },
};

let hasResults = false;
let lastReport = null;
let resultsFilter = "attention";
let progressVisible = false;
let qcProgressUnlisten = null;
let profileCatalog = [];
let selectedProfileNames = new Set(["youtube"]);
let activeResultsProfile = null;
let lastMultiReport = null;
let pendingProfileDelete = null;
let pendingProfileDeleteTimer = null;
let profileLibraryListenerReady = false;

const progressOverlay = document.getElementById("progress-overlay");
const progressTitle = document.getElementById("progress-title");
const progressMessage = document.getElementById("progress-message");
const progressDetail = document.getElementById("progress-detail");
const progressBar = document.getElementById("progress-bar");
const progressPhases = document.querySelectorAll(".progress-phases li");

const PHASE_ORDER = ["scan", "discover", "file", "finish"];

function getListen() {
  return window.__TAURI__?.event?.listen ?? null;
}

function setProgressPhases(activePhase, { doneThrough = null } = {}) {
  const activeIndex = PHASE_ORDER.indexOf(activePhase);
  const doneIndex = doneThrough ? PHASE_ORDER.indexOf(doneThrough) : activeIndex - 1;

  progressPhases.forEach((item) => {
    const phase = item.dataset.phase;
    const index = PHASE_ORDER.indexOf(phase);
    item.classList.remove("active", "done");

    if (index <= doneIndex) {
      item.classList.add("done");
    } else if (index === activeIndex) {
      item.classList.add("active");
    }
  });
}

function showProgress({ title = "Working…", message = "Please wait", detail = "", indeterminate = true } = {}) {
  if (!progressOverlay) {
    return;
  }

  progressVisible = true;
  progressTitle.textContent = title;
  progressMessage.textContent = message;
  progressDetail.textContent = detail;
  progressBar.classList.toggle("indeterminate", indeterminate);
  if (indeterminate) {
    progressBar.style.width = "";
  }
  progressOverlay.classList.remove("hidden");
  progressOverlay.setAttribute("aria-hidden", "false");
}

function hideProgress({ force = false } = {}) {
  if (!progressOverlay) {
    return;
  }

  if (!force && !progressVisible) {
    return;
  }

  progressVisible = false;
  progressOverlay.classList.add("hidden");
  progressOverlay.setAttribute("aria-hidden", "true");
  progressPhases.forEach((item) => item.classList.remove("active", "done"));
  progressBar.classList.add("indeterminate");
  progressBar.style.width = "";
}

function updateQcProgress(payload) {
  if (!progressVisible) {
    return;
  }

  const { phase, message, current, total } = payload;

  progressMessage.textContent = message;

  if (phase === "scan" || phase === "scanned") {
    progressTitle.textContent = "Scanning media";
    setProgressPhases("scan");
  } else if (phase === "discover" || phase === "start") {
    progressTitle.textContent = "Preparing check";
    setProgressPhases("discover", { doneThrough: "scan" });
  } else if (phase === "file" || phase === "profile") {
    progressTitle.textContent = phase === "profile" ? "Switching profile" : "Running QC";
    setProgressPhases("file", { doneThrough: "discover" });
    if (current != null && total != null && total > 0) {
      progressBar.classList.remove("indeterminate");
      progressBar.style.width = `${Math.round((current / total) * 100)}%`;
      progressDetail.textContent = payload.profile
        ? `${payload.profile} · file ${current} of ${total}`
        : `File ${current} of ${total}`;
    } else if (payload.profile) {
      progressDetail.textContent = payload.profile;
    }
  } else if (phase === "finish" || phase === "done") {
    progressTitle.textContent = "Finishing up";
    setProgressPhases("finish", { doneThrough: "file" });
    progressBar.classList.remove("indeterminate");
    progressBar.style.width = "100%";
    progressDetail.textContent = total ? `${total} file(s) checked` : "";
  }
}

async function setupQcProgressListener() {
  const listen = getListen();
  if (!listen || qcProgressUnlisten) {
    return;
  }

  try {
    qcProgressUnlisten = await listen("qc-progress", (event) => {
      updateQcProgress(event.payload);
    });
  } catch {
    /* Progress events are optional; the overlay still shows while work runs. */
  }
}

async function withProgress(task, options) {
  showProgress(options);
  try {
    return await task();
  } finally {
    hideProgress({ force: true });
  }
}

const workflowSteps = document.querySelectorAll(".workflow-step");
const screens = document.querySelectorAll(".screen");

function setStatus(el, message, kind = "muted") {
  el.textContent = message;
  el.className = `status-line ${kind}`;
}

function goToStep(stepId, { force = false } = {}) {
  const navStep = document.querySelector(`.workflow-step[data-step="${stepId}"]`);
  const screen = document.getElementById(stepId);

  if (!force && navStep?.classList.contains("locked")) {
    return;
  }

  workflowSteps.forEach((s) => s.classList.remove("active"));
  screens.forEach((s) => s.classList.remove("active"));

  navStep?.classList.add("active");
  screen?.classList.add("active");

  const meta = SCREENS[stepId];
  if (meta) {
    document.getElementById("screen-eyebrow").textContent = meta.eyebrow;
    document.getElementById("screen-title").textContent = meta.title;
    document.getElementById("screen-hint").textContent = meta.hint;
  }

  if (stepId === "check") {
    renderSelectedProfileChips(profileCatalog);
  }
}

function unlockStep(stepId) {
  document.querySelector(`[data-step="${stepId}"]`)?.classList.remove("locked");
}

function markStepDone(stepId) {
  const el = document.querySelector(`[data-step="${stepId}"]`);
  el?.classList.add("done");
  el?.classList.remove("locked");
}

function statusLabel(status) {
  const labels = {
    pass: "Ready to deliver",
    warn: "Review recommended",
    fail: "Not ready",
  };
  return labels[String(status).toLowerCase()] ?? status;
}

function fileNeedsAttention(file) {
  if (file.error) {
    return true;
  }
  const status = String(file.status).toLowerCase();
  return status === "fail" || status === "warn";
}

function sortFilesForDisplay(files) {
  const rank = (file) => {
    if (file.error) {
      return 0;
    }
    const status = String(file.status).toLowerCase();
    if (status === "fail") {
      return 1;
    }
    if (status === "warn") {
      return 2;
    }
    return 3;
  };

  return [...files].sort((a, b) => {
    const byRank = rank(a) - rank(b);
    if (byRank !== 0) {
      return byRank;
    }
    const nameA = a.path.split("/").pop() || a.path;
    const nameB = b.path.split("/").pop() || b.path;
    return nameA.localeCompare(nameB);
  });
}

function batchVerdict(summary, batchFindings = []) {
  const batchFail = batchFindings.some((f) => f.severity === "fail");
  const batchWarn = batchFindings.some((f) => f.severity === "warn");

  if (summary.failed > 0 || summary.errors > 0 || batchFail) {
    return {
      kind: "fail",
      title: "Not ready for delivery",
      hint: "Fix failed checks before you ship this batch.",
    };
  }
  if (summary.warned > 0 || batchWarn) {
    return {
      kind: "warn",
      title: "Review before delivery",
      hint: "Everything probed OK, but some files have warnings worth a quick look.",
    };
  }
  return {
    kind: "pass",
    title: "Ready for delivery",
    hint: "All files passed the selected profile rules.",
  };
}

function renderResultsVerdict(report) {
  const batchFindings = report.batch_findings || [];
  const verdict = batchVerdict(report.summary, batchFindings);
  const s = report.summary;
  const profile = escapeHtml(report.profile);

  document.getElementById("results-verdict").innerHTML = `
    <div class="verdict-card verdict-${verdict.kind}">
      <div class="verdict-copy">
        <p class="verdict-eyebrow">Batch verdict · ${profile} profile</p>
        <h3 class="verdict-title">${verdict.title}</h3>
        <p class="verdict-hint">${verdict.hint}</p>
      </div>
      <div class="verdict-stats">
        <div class="verdict-stat pass"><span class="num">${s.passed}</span><span class="lbl">Passed</span></div>
        <div class="verdict-stat warn"><span class="num">${s.warned}</span><span class="lbl">Warnings</span></div>
        <div class="verdict-stat fail"><span class="num">${s.failed + s.errors}</span><span class="lbl">Failed</span></div>
        <div class="verdict-stat"><span class="num">${s.total}</span><span class="lbl">Total</span></div>
      </div>
    </div>
  `;
}

function renderResultsSummary(report) {
  const s = report.summary;
  const total = s.total || 1;
  const passPct = Math.round((s.passed / total) * 100);
  const warnPct = Math.round((s.warned / total) * 100);
  const failPct = Math.round(((s.failed + s.errors) / total) * 100);

  document.getElementById("results-summary").innerHTML = `
    <div class="distribution-card card">
      <div class="distribution-header">
        <span>Batch breakdown</span>
        <span class="muted">${s.total} file${s.total === 1 ? "" : "s"} checked</span>
      </div>
      <div class="distribution-bar" aria-hidden="true">
        ${s.passed ? `<span class="segment pass" style="width:${passPct}%"></span>` : ""}
        ${s.warned ? `<span class="segment warn" style="width:${warnPct}%"></span>` : ""}
        ${s.failed + s.errors ? `<span class="segment fail" style="width:${failPct}%"></span>` : ""}
      </div>
      <div class="distribution-legend">
        <span><i class="dot pass"></i> ${s.passed} passed</span>
        <span><i class="dot warn"></i> ${s.warned} warnings</span>
        <span><i class="dot fail"></i> ${s.failed + s.errors} failed</span>
      </div>
    </div>
  `;
}

function renderBatchFindings(report) {
  const el = document.getElementById("batch-findings");
  const batchFindings = report.batch_findings || [];

  if (!batchFindings.length) {
    el.classList.add("hidden");
    el.innerHTML = "";
    return;
  }

  el.classList.remove("hidden");
  el.innerHTML = `
    <div class="card batch-findings-card">
      <h3>Batch issues</h3>
      <p class="field-help">These apply to the whole export folder, not a single file.</p>
      ${renderFindingList(batchFindings)}
    </div>
  `;
}

function findingLabel(code) {
  const labels = {
    BLACK_FRAMES: "Black frames",
    FROZEN_FRAMES: "Frozen picture",
    LOUDNESS_TOO_HIGH: "Loudness",
    TRUE_PEAK_TOO_HIGH: "True peak",
    VIDEO_CODEC_MISMATCH: "Video codec",
    FRAME_RATE_MISMATCH: "Frame rate",
    BATCH_MIXED_RESOLUTION: "Mixed resolutions",
    BATCH_MIXED_CODEC: "Mixed codecs",
    CAPTIONS_SHORT: "Captions coverage",
    CAPTIONS_MALFORMED: "Captions format",
  };
  return labels[code] ?? code;
}

function renderFindingList(findings) {
  if (!findings.length) {
    return "";
  }

  const fails = findings.filter((f) => f.severity === "fail");
  const warns = findings.filter((f) => f.severity === "warn");

  const renderGroup = (title, items, severity) => {
    if (!items.length) {
      return "";
    }
    return `
      <div class="issue-group">
        <h5 class="issue-group-title ${severity}">${title}</h5>
        <ul class="issue-list">
          ${items
            .map(
              (f) => `
            <li class="issue-item ${severity}">
              <span class="issue-code">${escapeHtml(findingLabel(f.code))}</span>
              <span class="issue-message">${escapeHtml(f.message)}</span>
            </li>`
            )
            .join("")}
        </ul>
      </div>`;
  };

  return `
    <div class="issues-block">
      ${renderGroup("Must fix", fails, "fail")}
      ${renderGroup("Warnings", warns, "warn")}
    </div>`;
}

function renderFileCard(file) {
  const basename = file.path.split("/").pop() || file.path;
  const status = String(file.status).toLowerCase();
  const issueCount = file.findings.length + (file.error ? 1 : 0);
  const issueLabel =
    issueCount === 0
      ? "No issues"
      : `${issueCount} issue${issueCount === 1 ? "" : "s"}`;
  const openByDefault = fileNeedsAttention(file);

  const suggestions =
    file.suggestions?.length > 0
      ? `
    <div class="fix-block">
      <h5 class="fix-title">Suggested fixes</h5>
      <ul class="fix-list">
        ${file.suggestions
          .map((item) => `<li>${escapeHtml(item.suggestion)}</li>`)
          .join("")}
      </ul>
    </div>`
      : "";

  const error = file.error
    ? `
    <div class="issue-group">
      <h5 class="issue-group-title fail">Could not check file</h5>
      <p class="issue-message">${escapeHtml(file.error)}</p>
    </div>`
    : "";

  const body =
    error ||
    (file.findings.length ? renderFindingList(file.findings) : "") ||
    `<p class="result-ok">No issues found — this file matches the profile.</p>`;

  return `
    <details class="result-file status-${status}" ${openByDefault ? "open" : ""}>
      <summary class="result-file-summary">
        <span class="result-status-dot" aria-hidden="true"></span>
        <span class="result-file-main">
          <span class="result-file-name">${escapeHtml(basename)}</span>
          <span class="result-file-meta">${issueLabel}</span>
        </span>
        <span class="badge ${status}">${statusLabel(status)}</span>
      </summary>
      <div class="result-file-body">
        <p class="result-path muted" title="${escapeHtml(file.path)}">${escapeHtml(file.path)}</p>
        ${body}
        ${suggestions}
      </div>
    </details>`;
}

function filesForFilter(files, filter) {
  if (filter === "pass") {
    return files.filter((file) => !fileNeedsAttention(file));
  }
  if (filter === "attention") {
    return files.filter((file) => fileNeedsAttention(file));
  }
  return files;
}

function renderResultsFileList() {
  if (!lastReport) {
    return;
  }

  const files = sortFilesForDisplay(lastReport.files);
  const visible = filesForFilter(files, resultsFilter);
  const filesEl = document.getElementById("file-results");
  const meta = document.getElementById("results-filter-meta");

  if (files.length === 0) {
    filesEl.innerHTML = `<div class="card empty-results"><p class="muted">No media files were found in that path.</p></div>`;
    meta.textContent = "";
    return;
  }

  if (visible.length === 0) {
    filesEl.innerHTML = `<div class="card empty-results"><p class="muted">No files match this filter.</p></div>`;
  } else {
    filesEl.innerHTML = visible.map(renderFileCard).join("");
  }

  const filterLabels = {
    attention: "Needs attention",
    pass: "Passed",
    all: "All files",
  };
  meta.textContent = `Showing ${visible.length} of ${files.length} · ${filterLabels[resultsFilter]}`;
}

function renderResultsFilters() {
  if (!lastReport) {
    return;
  }

  const files = lastReport.files;
  const attentionCount = files.filter((file) => fileNeedsAttention(file)).length;
  const passCount = files.length - attentionCount;

  const filters = [
    { id: "attention", label: "Needs attention", count: attentionCount },
    { id: "pass", label: "Passed", count: passCount },
    { id: "all", label: "All files", count: files.length },
  ];

  const filtersEl = document.getElementById("results-filters");
  filtersEl.innerHTML = filters
    .map(
      (filter) => `
      <button
        type="button"
        class="filter-btn ${resultsFilter === filter.id ? "active" : ""}"
        data-filter="${filter.id}"
        role="tab"
        aria-selected="${resultsFilter === filter.id}"
      >
        ${filter.label}
        <span class="filter-count">${filter.count}</span>
      </button>`
    )
    .join("");

  filtersEl.querySelectorAll(".filter-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      resultsFilter = btn.dataset.filter;
      renderResultsFilters();
      renderResultsFileList();
    });
  });
}

function renderReport(reportJson) {
  const payload = JSON.parse(reportJson);
  if (Array.isArray(payload.reports)) {
    renderMultiReport(payload);
  } else {
    renderSingleReport(payload);
  }
}

function renderSingleReport(report) {
  lastMultiReport = null;
  lastReport = report;
  activeResultsProfile = report.profile;
  hasResults = true;
  markStepDone("check");
  unlockStep("results");

  const attentionCount = report.files.filter((file) => fileNeedsAttention(file)).length;
  resultsFilter = attentionCount > 0 ? "attention" : "all";

  document.getElementById("results-empty").classList.add("hidden");
  document.getElementById("results-content").classList.remove("hidden");
  document.getElementById("profile-results-tabs").classList.add("hidden");
  document.getElementById("profile-results-tabs").innerHTML = "";

  renderActiveReport();
  goToStep("results", { force: true });
}

function renderMultiReport(multi) {
  lastMultiReport = multi;
  hasResults = true;
  markStepDone("check");
  unlockStep("results");

  const firstNeedsAttention = multi.reports.find(
    (report) => report.files.some((file) => fileNeedsAttention(file))
  );
  activeResultsProfile = (firstNeedsAttention || multi.reports[0])?.profile ?? null;
  lastReport = multi.reports.find((report) => report.profile === activeResultsProfile) || null;
  resultsFilter =
    lastReport && lastReport.files.some((file) => fileNeedsAttention(file))
      ? "attention"
      : "all";

  document.getElementById("results-empty").classList.add("hidden");
  document.getElementById("results-content").classList.remove("hidden");

  renderProfileResultsTabs(multi.reports);
  renderActiveReport();
  goToStep("results", { force: true });
}

function profileTabVerdictKind(report) {
  return batchVerdict(report.summary, report.batch_findings || []).kind;
}

function renderProfileResultsTabs(reports) {
  const tabsEl = document.getElementById("profile-results-tabs");
  if (reports.length <= 1) {
    tabsEl.classList.add("hidden");
    tabsEl.innerHTML = "";
    return;
  }

  tabsEl.classList.remove("hidden");
  tabsEl.innerHTML = reports
    .map((report) => {
      const kind = profileTabVerdictKind(report);
      const selected = report.profile === activeResultsProfile;
      return `
        <button
          type="button"
          class="profile-tab ${selected ? "active" : ""}"
          data-profile="${escapeHtml(report.profile)}"
          role="tab"
          aria-selected="${selected}"
        >
          <span>${escapeHtml(report.profile)}</span>
          <span class="profile-tab-badge ${kind}">${kind}</span>
        </button>`;
    })
    .join("");

  tabsEl.querySelectorAll(".profile-tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      activeResultsProfile = tab.dataset.profile;
      lastReport =
        lastMultiReport?.reports.find((report) => report.profile === activeResultsProfile) || null;
      const attentionCount =
        lastReport?.files.filter((file) => fileNeedsAttention(file)).length ?? 0;
      resultsFilter = attentionCount > 0 ? "attention" : "all";
      renderProfileResultsTabs(lastMultiReport.reports);
      renderActiveReport();
    });
  });
}

function renderActiveReport() {
  if (!lastReport) {
    return;
  }

  renderResultsVerdict(lastReport);
  renderResultsSummary(lastReport);
  renderBatchFindings(lastReport);
  renderResultsFilters();
  renderResultsFileList();
}

function getSelectedProfilesList() {
  return [...selectedProfileNames];
}

function isProfileSelected(name) {
  return selectedProfileNames.has(name);
}

function toggleProfileSelection(name) {
  if (selectedProfileNames.has(name)) {
    if (selectedProfileNames.size > 1) {
      selectedProfileNames.delete(name);
    }
  } else {
    selectedProfileNames.add(name);
  }
  syncProfileSelectionUI();
}

function syncProfileSelectionUI() {
  renderProfileLibrary(profileCatalog);
  renderSelectedProfileChips(profileCatalog);
  renderProfileSelectionSummary();
}

function escapeHtml(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function groupProfiles(profiles) {
  const groups = new Map();
  for (const profile of profiles) {
    const group = profile.group || "Other";
    if (!groups.has(group)) {
      groups.set(group, []);
    }
    groups.get(group).push(profile);
  }
  return groups;
}

function renderProfileSelect(profiles) {
  const inheritsSelect = document.getElementById("new-profile-inherits");
  if (!inheritsSelect) {
    return;
  }

  if (!profiles.length) {
    inheritsSelect.innerHTML = "";
    return;
  }

  const groups = groupProfiles(profiles);
  const groupOrder = [
    "Long-form",
    "Vertical social",
    "Web & review",
    "Other",
    "Custom",
  ];

  inheritsSelect.innerHTML = groupOrder
    .filter((name) => groups.has(name))
    .map((groupName) => {
      const items = groups.get(groupName);
      return `
        <optgroup label="${escapeHtml(groupName)}">
          ${items
            .map(
              (profile) =>
                `<option value="${escapeHtml(profile.name)}">${escapeHtml(profile.label)}</option>`
            )
            .join("")}
        </optgroup>`;
    })
    .join("");

  const base = profiles.find((profile) => profile.name === "youtube") || profiles[0];
  if (base && inheritsSelect) {
    inheritsSelect.value = base.name;
  }
}

function renderSelectedProfileChips(profiles) {
  const chipsEl = document.getElementById("profile-selected-chips");
  if (!chipsEl) {
    return;
  }

  const selected = getSelectedProfilesList();
  if (!selected.length) {
    chipsEl.innerHTML = `<p class="muted">No profiles selected. Go to step 1 to choose delivery specs.</p>`;
    return;
  }

  chipsEl.innerHTML = selected
    .map((name) => {
      const profile = profiles.find((item) => item.name === name);
      const label = profile?.label || name;
      return `<span class="profile-chip">${escapeHtml(label)}</span>`;
    })
    .join("");
}

function renderProfileSelectionSummary() {
  const summaryEl = document.getElementById("profile-selection-summary");
  if (!summaryEl) {
    return;
  }

  const selected = getSelectedProfilesList();
  if (!profileCatalog.length) {
    summaryEl.textContent = "No profiles loaded yet.";
    summaryEl.className = "profile-selection-summary muted";
    return;
  }

  if (!selected.length) {
    summaryEl.textContent = "Select at least one profile for QC checks.";
    summaryEl.className = "profile-selection-summary error";
    return;
  }

  const labels = selected.map(
    (name) => profileCatalog.find((profile) => profile.name === name)?.label || name
  );
  summaryEl.textContent = `${selected.length} profile${selected.length === 1 ? "" : "s"} selected for QC: ${labels.join(", ")}`;
  summaryEl.className = "profile-selection-summary success";
}

function renderProfileLibrary(profiles) {
  const library = document.getElementById("profile-library");
  if (!library) {
    return;
  }
  if (!profiles.length) {
    library.innerHTML = `<p class="muted">No profiles loaded yet.</p>`;
    return;
  }

  const groups = groupProfiles(profiles);
  const groupOrder = [
    "Long-form",
    "Vertical social",
    "Web & review",
    "Other",
    "Custom",
  ];

  library.innerHTML = groupOrder
    .filter((name) => groups.has(name))
    .map((groupName) => {
      const items = groups.get(groupName);
      return `
        <div class="profile-group">
          <h4 class="profile-group-title">${escapeHtml(groupName)}</h4>
          <div class="profile-grid">
            ${items
              .map(
                (profile) => `
              <div class="profile-card-shell">
                <div class="profile-card-row">
                  <button
                    type="button"
                    class="profile-card ${isProfileSelected(profile.name) ? "selected" : ""}"
                    data-profile="${escapeHtml(profile.name)}"
                    aria-pressed="${isProfileSelected(profile.name)}"
                  >
                    <span class="profile-card-check" aria-hidden="true">${isProfileSelected(profile.name) ? "✓" : ""}</span>
                    <span class="profile-card-copy">
                      <span class="profile-card-name">${escapeHtml(profile.label)}</span>
                      <span class="profile-card-meta">
                        ${escapeHtml(profile.extension)} · ${profile.width}×${profile.height}
                        ${profile.custom ? " · custom" : ""}
                      </span>
                    </span>
                  </button>
                  ${
                    profile.custom
                      ? `<button
                          type="button"
                          class="profile-delete-btn"
                          data-profile="${escapeHtml(profile.name)}"
                          aria-label="Delete ${escapeHtml(profile.label)}"
                          title="Delete profile"
                        >Delete</button>`
                      : ""
                  }
                </div>
              </div>`
              )
              .join("")}
          </div>
        </div>`;
    })
    .join("");

  resetPendingProfileDelete();
}

function resetPendingProfileDelete() {
  pendingProfileDelete = null;
  if (pendingProfileDeleteTimer) {
    clearTimeout(pendingProfileDeleteTimer);
    pendingProfileDeleteTimer = null;
  }
  document.querySelectorAll(".profile-delete-btn.confirming").forEach((btn) => {
    btn.classList.remove("confirming");
    btn.textContent = "Delete";
    btn.disabled = false;
  });
}

function setupProfileLibraryActions() {
  const library = document.getElementById("profile-library");
  if (!library || profileLibraryListenerReady) {
    return;
  }
  profileLibraryListenerReady = true;

  library.addEventListener("click", (event) => {
    const deleteBtn = event.target.closest(".profile-delete-btn");
    if (deleteBtn) {
      event.preventDefault();
      event.stopPropagation();
      void handleDeleteProfileClick(deleteBtn);
      return;
    }

    const card = event.target.closest(".profile-card");
    if (card?.dataset.profile) {
      toggleProfileSelection(card.dataset.profile);
    }
  });
}

async function handleDeleteProfileClick(btn) {
  const status = document.getElementById("profile-status");
  const name = btn.dataset.profile;
  if (!name) {
    return;
  }

  const profile = profileCatalog.find((item) => item.name === name);
  const label = profile?.label || name;

  if (!profile?.custom) {
    setStatus(status, "Only custom profiles can be deleted.", "error");
    return;
  }

  if (pendingProfileDelete !== name) {
    resetPendingProfileDelete();
    pendingProfileDelete = name;
    btn.classList.add("confirming");
    btn.textContent = "Confirm?";
    pendingProfileDeleteTimer = setTimeout(() => {
      if (pendingProfileDelete === name) {
        resetPendingProfileDelete();
      }
    }, 5000);
    setStatus(status, `Click “Confirm?” on “${label}” to delete it.`, "warn");
    return;
  }

  resetPendingProfileDelete();
  btn.disabled = true;

  try {
    await invoke("delete_profile", { name });
    selectedProfileNames.delete(name);
    if (selectedProfileNames.size === 0) {
      const fallback =
        profileCatalog.find((item) => item.name === "youtube" && item.name !== name)?.name ||
        profileCatalog.find((item) => item.name !== name)?.name;
      if (fallback) {
        selectedProfileNames.add(fallback);
      }
    }
    await refreshProfiles();
    setStatus(status, `Deleted profile “${label}”.`, "success");
  } catch (err) {
    setStatus(status, String(err), "error");
  } finally {
    btn.disabled = false;
  }
}

function readOptionalNumber(id) {
  const raw = document.getElementById(id)?.value?.trim();
  if (!raw) {
    return undefined;
  }
  const value = Number(raw);
  return Number.isFinite(value) ? value : undefined;
}

function readOptionalText(id) {
  const raw = document.getElementById(id)?.value?.trim();
  return raw || undefined;
}

function setOptionalNumber(id, value) {
  const field = document.getElementById(id);
  if (!field) {
    return;
  }
  field.value = value == null ? "" : value;
}

function setOptionalText(id, value) {
  const field = document.getElementById(id);
  if (!field) {
    return;
  }
  field.value = value || "";
}

function applyProfileRules(rules) {
  if (!rules) {
    return;
  }

  document.getElementById("new-profile-require-audio").checked = Boolean(rules.requireAudio);
  document.getElementById("new-profile-require-captions").checked = Boolean(rules.requireCaptions);
  document.getElementById("new-profile-require-thumbnail").checked = Boolean(rules.requireThumbnail);
  document.getElementById("new-profile-min-duration").value = rules.minDurationSecs ?? 1;
  setOptionalNumber("new-profile-max-duration", rules.maxDurationSecs);
  document.getElementById("new-profile-max-silence").value = rules.maxSilenceSecs ?? 2;
  document.getElementById("new-profile-min-mean-volume").value = rules.minMeanVolumeDb ?? -50;
  setOptionalText("new-profile-video-codec", rules.videoCodec);
  setOptionalNumber("new-profile-frame-rate", rules.frameRate);
  document.getElementById("new-profile-frame-rate-tolerance").value =
    rules.frameRateTolerance ?? 0.05;
  setOptionalNumber("new-profile-min-video-bitrate", rules.minVideoBitrateKbps);
  setOptionalNumber("new-profile-max-video-bitrate", rules.maxVideoBitrateKbps);
  setOptionalText("new-profile-audio-codec", rules.audioCodec);
  setOptionalNumber("new-profile-audio-sample-rate", rules.audioSampleRate);
  setOptionalNumber("new-profile-audio-channels", rules.audioChannels);
  setOptionalNumber("new-profile-max-lufs", rules.maxIntegratedLufs);
  setOptionalNumber("new-profile-max-true-peak", rules.maxTruePeakDb);
  setOptionalNumber("new-profile-max-file-size", rules.maxFileSizeMb);
}

function applyProfileTemplate(profileName) {
  const profile = profileCatalog.find((item) => item.name === profileName);
  if (!profile) {
    return;
  }
  document.getElementById("new-profile-width").value = profile.width;
  document.getElementById("new-profile-height").value = profile.height;
  document.getElementById("new-profile-extension").value = profile.extension || "mp4";
  applyProfileRules(profile.rules);
}

function resetProfileFormDefaults() {
  document.getElementById("new-profile-width").value = 1920;
  document.getElementById("new-profile-height").value = 1080;
  document.getElementById("new-profile-extension").value = "mp4";
  applyProfileRules({
    requireAudio: true,
    requireCaptions: false,
    requireThumbnail: false,
    minDurationSecs: 1,
    maxDurationSecs: null,
    maxSilenceSecs: 2,
    minMeanVolumeDb: -50,
    videoCodec: null,
    frameRate: null,
    frameRateTolerance: 0.05,
    minVideoBitrateKbps: null,
    maxVideoBitrateKbps: null,
    audioCodec: null,
    audioSampleRate: null,
    audioChannels: null,
    maxIntegratedLufs: null,
    maxTruePeakDb: null,
    maxFileSizeMb: null,
  });
}

function buildCreateProfileInput(name, useTemplate) {
  const input = {
    name,
    extension: document.getElementById("new-profile-extension").value,
    width: Number(document.getElementById("new-profile-width").value),
    height: Number(document.getElementById("new-profile-height").value),
    requireAudio: document.getElementById("new-profile-require-audio").checked,
    requireCaptions: document.getElementById("new-profile-require-captions").checked,
    requireThumbnail: document.getElementById("new-profile-require-thumbnail").checked,
    minDurationSecs: readOptionalNumber("new-profile-min-duration"),
    maxDurationSecs: readOptionalNumber("new-profile-max-duration"),
    maxSilenceSecs: readOptionalNumber("new-profile-max-silence"),
    minMeanVolumeDb: readOptionalNumber("new-profile-min-mean-volume"),
    videoCodec: readOptionalText("new-profile-video-codec"),
    frameRate: readOptionalNumber("new-profile-frame-rate"),
    frameRateTolerance: readOptionalNumber("new-profile-frame-rate-tolerance"),
    minVideoBitrateKbps: readOptionalNumber("new-profile-min-video-bitrate"),
    maxVideoBitrateKbps: readOptionalNumber("new-profile-max-video-bitrate"),
    audioCodec: readOptionalText("new-profile-audio-codec"),
    audioSampleRate: readOptionalNumber("new-profile-audio-sample-rate"),
    audioChannels: readOptionalNumber("new-profile-audio-channels"),
    maxIntegratedLufs: readOptionalNumber("new-profile-max-lufs"),
    maxTruePeakDb: readOptionalNumber("new-profile-max-true-peak"),
    maxFileSizeMb: readOptionalNumber("new-profile-max-file-size"),
  };

  if (useTemplate) {
    input.inherits = document.getElementById("new-profile-inherits").value;
  }

  return input;
}

function syncProfileTemplateControls() {
  const useTemplate = document.getElementById("new-profile-use-template")?.checked;
  const templateField = document.getElementById("new-profile-template-field");
  const inheritsSelect = document.getElementById("new-profile-inherits");

  if (!templateField || !inheritsSelect) {
    return;
  }

  templateField.classList.toggle("hidden", !useTemplate);
  inheritsSelect.disabled = !useTemplate;

  if (useTemplate) {
    applyProfileTemplate(inheritsSelect.value || "youtube");
  }
}

async function refreshProfiles() {
  const status = document.getElementById("profile-status");
  try {
    profileCatalog = await invoke("list_profiles");
    if (
      selectedProfileNames.size === 0 ||
      ![...selectedProfileNames].some((name) => profileCatalog.some((p) => p.name === name))
    ) {
      selectedProfileNames = new Set(
        [
          profileCatalog.find((p) => p.name === "youtube")?.name || profileCatalog[0]?.name,
        ].filter(Boolean)
      );
    }
    renderProfileSelect(profileCatalog);
    syncProfileSelectionUI();
    syncProfileTemplateControls();
  } catch (err) {
    const message = String(err);
    if (status) {
      setStatus(
        status,
        message.includes("Desktop bridge")
          ? message
          : `Could not load profiles: ${message}`,
        "error"
      );
    }
    renderProfileSelectionSummary();
  }
}

workflowSteps.forEach((step) => {
  step.addEventListener("click", () => {
    if (!step.classList.contains("locked")) {
      goToStep(step.dataset.step, { force: true });
    }
  });
});

document.getElementById("continue-to-check-btn").addEventListener("click", () => {
  goToStep("check", { force: true });
});

document.getElementById("go-profiles-btn").addEventListener("click", () => {
  goToStep("profiles", { force: true });
});

document.getElementById("new-profile-use-template").addEventListener("change", () => {
  syncProfileTemplateControls();
});

document.getElementById("new-profile-inherits").addEventListener("change", (event) => {
  if (document.getElementById("new-profile-use-template").checked) {
    applyProfileTemplate(event.target.value);
  }
});

document.getElementById("create-profile-btn").addEventListener("click", async () => {
  const status = document.getElementById("profile-status");
  const btn = document.getElementById("create-profile-btn");
  const name = document.getElementById("new-profile-name").value.trim();
  const useTemplate = document.getElementById("new-profile-use-template").checked;
  const width = Number(document.getElementById("new-profile-width").value);
  const height = Number(document.getElementById("new-profile-height").value);

  if (!name) {
    setStatus(status, "Enter a profile ID (e.g. acme_youtube).", "error");
    document.getElementById("new-profile-name").focus();
    return;
  }

  if (!width || !height) {
    setStatus(status, "Width and height are required.", "error");
    return;
  }

  btn.disabled = true;

  try {
    const input = buildCreateProfileInput(name, useTemplate);
    const created = await invoke("create_profile", { input });
    selectedProfileNames.add(created.name);
    document.getElementById("new-profile-name").value = "";
    document.getElementById("new-profile-use-template").checked = false;
    syncProfileTemplateControls();
    resetProfileFormDefaults();
    await refreshProfiles();
    setStatus(status, `Saved profile “${created.label}”.`, "success");
  } catch (err) {
    setStatus(status, String(err), "error");
  } finally {
    btn.disabled = false;
  }
});

document.getElementById("run-local-btn").addEventListener("click", async () => {
  const status = document.getElementById("check-status");
  const btn = document.getElementById("run-local-btn");
  const path = document.getElementById("qc-path").value.trim();
  const profiles = getSelectedProfilesList();

  if (!path) {
    setStatus(status, "Enter the path to your export folder or file.", "error");
    document.getElementById("qc-path").focus();
    return;
  }

  if (!profiles.length) {
    setStatus(status, "Select at least one delivery profile.", "error");
    goToStep("check", { force: true });
    return;
  }

  btn.disabled = true;

  try {
    let reportJson;
    await withProgress(
      async () => {
        setStatus(status, "Running QC…");
        const result = await invoke("run_local_qc", { path, profiles });
        reportJson = result.report_json;
      },
      {
        title: "Running QC check",
        message: "Scanning your export and validating against delivery profiles…",
        detail: `${profiles.length} profile${profiles.length === 1 ? "" : "s"}: ${profiles.join(", ")}`,
      }
    );

    setStatus(status, "Check complete. Review your results on the next step.", "success");
    renderReport(reportJson);
  } catch (err) {
    setStatus(status, String(err), "error");
  } finally {
    btn.disabled = false;
  }
});

document.getElementById("back-to-profiles-btn").addEventListener("click", () => goToStep("profiles", { force: true }));
document.getElementById("back-to-check-btn").addEventListener("click", () => goToStep("check", { force: true }));
document.getElementById("go-check-btn").addEventListener("click", () => goToStep("check", { force: true }));
document.getElementById("run-another-btn").addEventListener("click", () => {
  document.getElementById("qc-path").value = "";
  document.getElementById("qc-path").focus();
  goToStep("check", { force: true });
});

let watchActive = false;
let watchListenersReady = false;

function renderWatchStatus(status) {
  watchActive = status.active;
  const pill = document.getElementById("watch-pill");
  const startBtn = document.getElementById("start-watch-btn");
  const stopBtn = document.getElementById("stop-watch-btn");
  const pathInput = document.getElementById("watch-path");
  const debounceInput = document.getElementById("watch-debounce");
  const scanExisting = document.getElementById("watch-scan-existing");
  const watchStatus = document.getElementById("watch-status");

  if (status.active) {
    pill.textContent = "Watching";
    pill.className = "watch-pill active";
    startBtn.disabled = true;
    stopBtn.disabled = false;
    if (status.path) {
      pathInput.value = status.path;
    }
    if (status.debounce_secs != null) {
      debounceInput.value = status.debounce_secs;
    }
    scanExisting.checked = Boolean(status.scan_existing);
    pathInput.disabled = true;
    debounceInput.disabled = true;
    scanExisting.disabled = true;
    const profileLabel = (status.profiles || []).join(", ");
    setStatus(
      watchStatus,
      profileLabel ? `Watching ${status.path} · ${profileLabel}` : `Watching ${status.path}`,
      "success"
    );
  } else {
    pill.textContent = "Inactive";
    pill.className = "watch-pill muted";
    startBtn.disabled = false;
    stopBtn.disabled = true;
    pathInput.disabled = false;
    debounceInput.disabled = false;
    scanExisting.disabled = false;
  }
}

function appendWatchActivity(message) {
  const list = document.getElementById("watch-activity");
  const item = document.createElement("li");
  const time = new Date().toLocaleTimeString();
  item.textContent = `${time} — ${message}`;
  list.prepend(item);
  while (list.children.length > 8) {
    list.removeChild(list.lastChild);
  }
}

async function setupWatchListeners() {
  const listen = getListen();
  if (!listen || watchListenersReady) {
    return;
  }

  await listen("watch-status", (event) => {
    appendWatchActivity(event.payload.message);
    if (watchActive) {
      setStatus(document.getElementById("watch-status"), event.payload.message, "muted");
    }
  });

  await listen("watch-report", (event) => {
    const { report_json, exit_code } = event.payload;
    appendWatchActivity(`QC complete (exit ${exit_code})`);
    renderReport(report_json);
  });

  watchListenersReady = true;
}

async function refreshWatchStatus() {
  try {
    const status = await invoke("get_watch_status");
    renderWatchStatus(status);
  } catch {
    /* bridge not ready yet */
  }
}

function syncWatchPathFromQcPath() {
  const qcPath = document.getElementById("qc-path").value.trim();
  const watchPath = document.getElementById("watch-path");
  if (qcPath && !watchPath.value.trim()) {
    watchPath.value = qcPath;
  }
}

document.getElementById("qc-path").addEventListener("input", syncWatchPathFromQcPath);

document.getElementById("start-watch-btn").addEventListener("click", async () => {
  const watchStatus = document.getElementById("watch-status");
  const path =
    document.getElementById("watch-path").value.trim() ||
    document.getElementById("qc-path").value.trim();
  const profiles = getSelectedProfilesList();
  const debounceSecs = Number(document.getElementById("watch-debounce").value) || 5;
  const scanExisting = document.getElementById("watch-scan-existing").checked;

  if (!path) {
    setStatus(watchStatus, "Enter a drop folder path to watch.", "error");
    document.getElementById("watch-path").focus();
    return;
  }

  if (!profiles.length) {
    setStatus(watchStatus, "Select at least one delivery profile.", "error");
    return;
  }

  document.getElementById("watch-path").value = path;

  try {
    const status = await invoke("start_watch_folder", {
      path,
      profiles,
      debounceSecs,
      scanExisting,
    });
    renderWatchStatus(status);
    appendWatchActivity(`Started watching ${path}`);
  } catch (err) {
    setStatus(watchStatus, String(err), "error");
  }
});

document.getElementById("stop-watch-btn").addEventListener("click", async () => {
  const watchStatus = document.getElementById("watch-status");
  try {
    const status = await invoke("stop_watch_folder");
    renderWatchStatus(status);
    appendWatchActivity("Watch stopped");
    setStatus(watchStatus, "Watch folder stopped.", "muted");
  } catch (err) {
    setStatus(watchStatus, String(err), "error");
  }
});

async function initDesktop() {
  try {
    setupProfileLibraryActions();
    await waitForTauri();
    await setupQcProgressListener();
    await setupWatchListeners();
    await refreshWatchStatus();
    await refreshProfiles();
    resetProfileFormDefaults();
    syncProfileTemplateControls();
    goToStep("profiles", { force: true });
  } catch (err) {
    setStatus(document.getElementById("profile-status"), String(err), "error");
  } finally {
    hideProgress({ force: true });
  }
}

initDesktop();
