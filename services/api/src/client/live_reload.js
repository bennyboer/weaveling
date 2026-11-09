function (port) {
  const protocol = location.protocol === "https:" ? "wss://" : "ws://";

  function connect() {
    const told = new WebSocket(`${protocol}${location.hostname}:${port}/live_reload`);

    told.onmessage = (event) => {
      const change = JSON.parse(event.data);
      if (change.all) {
        location.reload();
      }
      if (change.css) {
        document.querySelectorAll("link[rel=stylesheet]").forEach((link) => {
          if (link.getAttribute("href").includes(change.css)) {
            link.setAttribute("href", `/${change.css}?version=${Date.now()}`);
          }
        });
      }
    };
    told.onclose = () => setTimeout(connect, 1000);
    told.onerror = () => told.close();
  }

  connect();
}
