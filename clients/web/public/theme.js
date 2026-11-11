try {
  var said = localStorage.getItem("weaveling.theme");
  if (said === "light" || said === "dark") {
    document.documentElement.setAttribute("data-theme", said);
  }
} catch (beyondReach) {}
