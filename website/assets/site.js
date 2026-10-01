(function () {
  "use strict";

  // Mobile nav
  var burger = document.getElementById("burger");
  var navLinks = document.querySelector(".nav nav");
  if (burger && navLinks) {
    burger.addEventListener("click", function () {
      navLinks.classList.toggle("open");
    });
  }

  // Install tabs
  var tabs = document.querySelectorAll("#install-tabs .tab");
  var panes = document.querySelectorAll(".install .pane");
  tabs.forEach(function (tab) {
    tab.addEventListener("click", function () {
      tabs.forEach(function (t) { t.classList.remove("active"); });
      panes.forEach(function (p) { p.classList.remove("active"); });
      tab.classList.add("active");
      var pane = document.querySelector('.install .pane[data-os="' + tab.dataset.os + '"]');
      if (pane) { pane.classList.add("active"); }
    });
  });

  // Sidebar: mark the current page
  var here = location.pathname.split("/").pop() || "index.html";
  document.querySelectorAll(".side a").forEach(function (link) {
    if (link.getAttribute("href") === here) { link.classList.add("on"); }
  });

  // Scroll spy for in-page headings
  var heads = [].slice.call(document.querySelectorAll(".prose h2[id], .prose h3[id]"));
  var sideLinks = [].slice.call(document.querySelectorAll(".toc a"));
  if (heads.length && sideLinks.length && "IntersectionObserver" in window) {
    var seen = {};
    var spy = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) { seen[entry.target.id] = true; }
      });
      var current = heads[heads.length - 1];
      heads.forEach(function (h) { if (seen[h.id]) { current = h; } });
      sideLinks.forEach(function (a) {
        a.style.fontWeight = a.getAttribute("href") === "#" + current.id ? "700" : "";
        a.style.color = a.getAttribute("href") === "#" + current.id ? "var(--primary)" : "";
      });
    }, { rootMargin: "-70px 0px -75% 0px" });
    heads.forEach(function (h) { spy.observe(h); });
  }

  // Docs search: matches the prebuilt index when present
  var input = document.getElementById("q");
  var out = document.getElementById("results");
  if (input && out && window.AEC_INDEX) {
    var index = window.AEC_INDEX;
    input.addEventListener("input", function () {
      var term = input.value.trim().toLowerCase();
      out.innerHTML = "";
      if (term.length < 2) { out.classList.remove("on"); return; }
      var hits = [];
      for (var i = 0; i < index.length; i++) {
        var entry = index[i];
        var hay = (entry.title + " " + entry.text).toLowerCase();
        var at = hay.indexOf(term);
        if (at === -1) { continue; }
        var titleAt = entry.title.toLowerCase().indexOf(term);
        hits.push({
          score: (titleAt === -1 ? 1000 : 0) + at,
          url: entry.url + (entry.anchor ? "#" + entry.anchor : ""),
          title: entry.title,
          snippet: snippet(entry.text, at)
        });
      }
      hits.sort(function (a, b) { return a.score - b.score; });
      hits = hits.slice(0, 12);
      if (!hits.length) {
        out.innerHTML = '<a><b>No matches</b><span>Try a different term.</span></a>';
      } else {
        hits.forEach(function (hit) {
          var a = document.createElement("a");
          a.href = hit.url;
          var b = document.createElement("b");
          b.innerHTML = mark(hit.title, term);
          var s = document.createElement("span");
          s.innerHTML = mark(hit.snippet, term);
          a.appendChild(b);
          a.appendChild(s);
          out.appendChild(a);
        });
      }
      out.classList.add("on");
    });
  }

  function snippet(text, at) {
    var start = Math.max(0, at - 60);
    var raw = text.slice(start, start + 150);
    return (start > 0 ? "…" : "") + raw + "…";
  }

  function mark(value, term) {
    var safe = value.replace(/[&<>"]/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c];
    });
    var idx = safe.toLowerCase().indexOf(term);
    if (idx === -1) { return safe; }
    var pre = safe.slice(0, idx);
    var hit = safe.slice(idx, idx + term.length);
    var post = safe.slice(idx + term.length);
    return pre + "<mark>" + hit + "</mark>" + post;
  }

  // Copy buttons on code blocks
  document.querySelectorAll(".prose pre").forEach(function (pre) {
    var button = document.createElement("button");
    button.className = "copy";
    button.textContent = "Copy";
    button.addEventListener("click", function () {
      var text = pre.innerText;
      if (navigator.clipboard) {
        navigator.clipboard.writeText(text).then(function () {
          button.textContent = "Copied";
          setTimeout(function () { button.textContent = "Copy"; }, 1400);
        });
      }
    });
    pre.style.position = "relative";
    pre.appendChild(button);
  });
})();
