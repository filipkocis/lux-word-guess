function updateGames() {
  fetch("/games").then(res => {
    if (res.ok) {
      res.json().then(res => {
        const games = res.games

        if (window.games) {
          if (window.games.length === games.length) {
            for (let i = 0; i < games.length; i++) {
              if (window.games[i].id !== games[i].id) {
                break;
              }
            }
            return
          }
        }

        window.games = games
        const gamesEl = document.getElementById("games")
        gamesEl.innerHTML = ''
        gamesEl.innerHTML = games.map(gameHtml).join('')
      })
    }
  })
}

function gameHtml(game) {
  const winner = (id) => {
    if (id === game.winner) {
      return "text-lime-500"
    } else {
      return "text-red-500"
    }
  }

  const lines = (arr) => {
    return arr.map(line => line).join("<br>")
  }

  return `
<div class="flex flex-col gap-2 border-4 rounded-xl p-2 min-w-[400px]">
  <p class="text-center text-xl">
    <span class="${winner(game.hinter)}"> Hinter ${game.hinter}</span>
    vs 
    <span class="${winner(game.guesser)}">Guesser ${game.guesser}</span>
  </p> 
  <div class="[&>p]:text-gray-500 text-start gap-x-4 grid grid-cols-[60px_auto]">
    <p>Finished</p><div>${new Date(game.timestamp).toLocaleTimeString()}</div>
    <p>Word</p><div>${game.word}</div>
    <p>Hints</p><div>${lines(game.hints)}</div>
    <p>Guesses</p><div>${lines(game.guesses)}</div>
  </div>
</div>
`
}

updateGames()
setInterval(() => {
  updateGames()
}, 5000)
