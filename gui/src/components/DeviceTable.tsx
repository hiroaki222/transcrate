import type { DeviceRow } from "../api";
import { useStrings } from "../strings";

const COLUMNS = ["MP3", "AAC", "WAV", "AIFF", "FLAC", "ALAC"];

const khz = (hz: number | null) => (hz === null ? "—" : `${hz / 1000}k`);

type Props = { rows: DeviceRow[] };

/**
 * The numbers behind every warning.
 *
 * Newer does not mean more capable — a 2016 CDJ-2000NXS2 plays 96 kHz FLAC and
 * a 2026 XDJ-AN stops at 48 kHz — so the rows carry their release year and are
 * never ranked.
 */
export function DeviceTable({ rows }: Props) {
  const t = useStrings();


  return (
    <div className="pane">
      <div className="grid-wrap">
        <table className="grid">
          <thead>
            <tr>
              <th scope="col">DEVICE</th>
              <th scope="col">YEAR</th>
              {COLUMNS.map((column) => (
                <th scope="col" key={column}>
                  {column}
                </th>
              ))}
              <th scope="col">exFAT</th>
              <th scope="col">DEPTH</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.id}>
                <th scope="row">{row.name}</th>
                <td>{row.year}</td>
                {row.ratesHz.map((hz, at) => (
                  <td key={COLUMNS[at]} className={hz === null ? "no" : ""}>
                    {khz(hz)}
                  </td>
                ))}
                <td className={row.exfat ? "yes" : "no"}>
                  {row.exfat ? t.devices.yes : t.devices.no}
                </td>
                <td>{row.maxFolderDepth}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="note">{t.devices.source}</p>
    </div>
  );
}
