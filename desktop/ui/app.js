const invoke = window.__TAURI__?.core?.invoke ?? (async () => {
  throw new Error("Tauri runtime not available");
});

const panels = document.querySelectorAll(".panel");
const steps = document.querySelectorAll(".step");

steps.forEach((step) => {
  step.addEventListener("click", () => {
    steps.forEach((s) => s.classList.remove("active"));
    panels.forEach((p) => p.classList.remove("active"));
    step.classList.add("active");
    document.getElementById(step.dataset.step).classList.add("active");
  });
});

function renderEntitlements(data) {
  const el = document.getElementById("entitlements");
  el.innerHTML = `<strong>Plan:</strong> ${data.plan}<br/><strong>Usage:</strong> ${JSON.stringify(data.usage, null, 2)}`;
  const banner = document.getElementById("upgrade-banner");
  if (data.plan === "free") {
    banner.classList.remove("hidden");
    banner.textContent = "Upgrade to Pro for parallel batch, remote jobs, and HTML reports.";
  } else {
    banner.classList.add("hidden");
  }
}

function renderReport(reportJson) {
  const report = JSON.parse(reportJson);
  const lines = [`Profile: ${report.profile}`, `Summary: ${JSON.stringify(report.summary)}`, ""];
  for (const file of report.files) {
    lines.push(`${file.path} — ${file.status} / ${file.review_state}`);
    for (const finding of file.findings) {
      lines.push(`  [${finding.severity}] ${finding.code}: ${finding.message}`);
    }
    for (const suggestion of file.suggestions) {
      lines.push(`  → ${suggestion.suggestion}`);
    }
    if (file.error) lines.push(`  ERROR: ${file.error}`);
    lines.push("");
  }
  document.getElementById("results-output").textContent = lines.join("\n");
  document.querySelector('[data-step="results"]').click();
}

async function refreshQueue() {
  const queue = await invoke("get_offline_queue");
  const list = document.getElementById("offline-queue");
  list.innerHTML = queue
    .map((item) => `<li>${item.id} — ${item.kind} — synced: ${item.synced}</li>`)
    .join("");
}

document.getElementById("login-btn").addEventListener("click", async () => {
  const token = document.getElementById("api-token").value;
  await invoke("login", { apiToken: token });
  const workspaces = await invoke("list_workspaces");
  const select = document.getElementById("workspace-select");
  select.innerHTML = workspaces
    .map((w) => `<option value="${w.id}">${w.name} (${w.plan})</option>`)
    .join("");
  const entitlements = await invoke("get_entitlements");
  renderEntitlements(entitlements);
  const variant = await invoke("get_pricing_variant");
  document.getElementById("pricing-variant").textContent = `${variant.headline} — ${variant.cta}`;
});

document.getElementById("switch-workspace-btn").addEventListener("click", async () => {
  const workspaceId = document.getElementById("workspace-select").value;
  await invoke("switch_workspace", { workspaceId });
  const entitlements = await invoke("get_entitlements");
  renderEntitlements(entitlements);
});

document.getElementById("run-local-btn").addEventListener("click", async () => {
  const path = document.getElementById("qc-path").value;
  const profile = document.getElementById("profile-select").value;
  const result = await invoke("run_local_qc", { path, profile });
  renderReport(result.report_json);
});

document.getElementById("queue-remote-btn").addEventListener("click", async () => {
  const profile = document.getElementById("profile-select").value;
  await invoke("queue_offline_action", {
    kind: "remote_job",
    payload: { upload_id: "upload_demo-studio_0", profile },
  });
  await refreshQueue();
  document.querySelector('[data-step="queue"]').click();
});

document.getElementById("sync-queue-btn").addEventListener("click", async () => {
  await invoke("sync_offline_queue");
  await refreshQueue();
});

refreshQueue().catch(() => {});
