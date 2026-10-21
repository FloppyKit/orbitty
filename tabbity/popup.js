const note = document.querySelector("#note");
const planet = document.querySelector("#planet");

function say(text) {
  note.textContent = text;
}

chrome.runtime.sendMessage({ type: "planets" }, (res) => {
  const rows = res?.planets || [];
  planet.replaceChildren();
  if (!rows.length) {
    say(res?.error || "No planets, or the browser isn't connected.");
    return;
  }
  for (const p of rows) {
    const opt = document.createElement("option");
    opt.value = p.id;
    opt.textContent = p.name;
    planet.append(opt);
  }
});

function claim(type) {
  chrome.runtime.sendMessage({ type, projectId: planet.value }, (res) => {
    say(res?.error || (res?.ok ? "Claimed." : "No reply."));
  });
}

document.querySelector("#claim").onclick = () => claim("claim");
document.querySelector("#all").onclick = () => claim("claim-all");
document.querySelector("#release").onclick = () => {
  chrome.runtime.sendMessage({ type: "release" }, (res) => {
    say(res?.error || (res?.ok ? "Released." : "No reply."));
  });
};
