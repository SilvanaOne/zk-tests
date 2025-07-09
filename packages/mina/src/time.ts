export function formatTime(time: number) {
  const ms = time % 1000;
  const minutes = Math.floor(time / 60000);
  const seconds = Math.floor((time % 60000) / 1000);
  return `${minutes === 0 ? "" : `${minutes} min `}${
    seconds === 0 ? "" : `${seconds} sec`
  }${seconds === 0 && minutes === 0 ? `${ms} ms` : ""}`;
}
