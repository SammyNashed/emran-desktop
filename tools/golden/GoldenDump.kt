// Dumps reference prayer times and the Umm al-Qura table from the Android app's real
// PrayerTimeCalculator.kt, so the Rust port is compared with the app and not with itself.
import io.github.sammynashed.emran.data.PrayerTimeCalculator as P
import java.io.File
import java.time.LocalDate
import java.time.ZoneId
import java.time.chrono.HijrahDate
import java.time.temporal.ChronoField

private data class Place(val lat: Double, val lon: Double, val zone: String)

fun main(args: Array<String>) {
    val root = File(args[0]); val out = File(root, "tests/data"); out.mkdirs(); val dataDir = File(root, "data"); dataDir.mkdirs()
    val aleppo = Place(36.2021, 37.1343, "Asia/Damascus")
    val places = listOf(
        aleppo, Place(36.40, 37.35, "Asia/Damascus"), Place(33.5138, 36.2913, "Asia/Damascus"),
        Place(41.8781, -87.6298, "America/Chicago"), Place(-36.8485, 174.7633, "Pacific/Auckland"),
        Place(59.3293, 18.0686, "Europe/Stockholm"), Place(69.6492, 18.9553, "Europe/Oslo"),
        Place(33.5731, -7.5898, "Africa/Casablanca"), Place(3.1390, 101.6869, "Asia/Kuala_Lumpur"),
    )
    val dates = listOf("2026-01-15", "2026-03-08", "2026-06-21", "2026-11-01", "2026-11-08", "2027-02-28", "2028-02-29").map(LocalDate::parse)
    val customs = listOf(P.CustomParameters(16.5, 15.0, 0, 1.5), P.CustomParameters(19.0, 0.0, 75, 0.0))

    val sb = StringBuilder("date,lat,lon,zone,method,school,rule,adj,custom,ramadan,fajr,sunrise,dhuhr,asr,maghrib,isha,sun,twilight\n")
    fun row(d: LocalDate, p: Place, m: P.CalculationMethod, s: P.AsrSchool, r: P.HighLatitudeRule, adj: List<Int>, c: P.CustomParameters?, ram: Boolean) {
        val a = P.Adjustments(adj[0], adj[1], adj[2], adj[3], adj[4], adj[5])
        val t = P.calculateTimes(d, p.lat, p.lon, ZoneId.of(p.zone), m, s, r, a, c, ram)
        val cs = c?.let { "${it.fajrAngle}|${it.ishaAngle}|${it.ishaIntervalMinutes}|${it.maghribAngle}" } ?: ""
        sb.append("$d,${p.lat},${p.lon},${p.zone},$m,$s,$r,${adj.joinToString("|")},$cs,$ram,")
        fun f(x: java.time.LocalDateTime) = x.toString().let { if (it.length == 16) "$it:00" else it }
        sb.append("${f(t.fajr)},${f(t.sunrise)},${f(t.dhuhr)},${f(t.asr)},${f(t.maghrib)},${f(t.isha)},${t.hasTrueSunriseAndSunset},${t.twilightIsAstronomical}\n")
    }
    val none = listOf(0, 0, 0, 0, 0, 0)
    // Every day for two years at Aleppo with the app's defaults.
    var d = LocalDate.parse("2026-01-01")
    while (d.isBefore(LocalDate.parse("2028-01-01"))) { row(d, aleppo, P.CalculationMethod.MWL, P.AsrSchool.SHAFII, P.HighLatitudeRule.AUTOMATIC, none, null, false); d = d.plusDays(1) }
    // Every method, school and rule across the awkward places and dates.
    for (p in places) for (day in dates) for (m in P.CalculationMethod.values()) for (s in P.AsrSchool.values()) for (r in P.HighLatitudeRule.values()) {
        if (m == P.CalculationMethod.CUSTOM) customs.forEach { row(day, p, m, s, r, none, it, false) } else row(day, p, m, s, r, none, null, false)
    }
    // Fine-tune offsets and the Ramadan interval.
    for (p in places) for (day in dates) {
        row(day, p, P.CalculationMethod.MWL, P.AsrSchool.SHAFII, P.HighLatitudeRule.AUTOMATIC, listOf(2, -3, 1, 90, -45, 7), null, false)
        row(day, p, P.CalculationMethod.UMM_AL_QURA, P.AsrSchool.HANAFI, P.HighLatitudeRule.AUTOMATIC, none, null, true)
    }
    File(out, "golden_times.csv").writeText(sb.toString())

    val h = StringBuilder()
    val range = java.time.chrono.HijrahChronology.INSTANCE.range(ChronoField.YEAR)
    val firstY = range.minimum.toInt(); val lastY = range.maximum.toInt()
    h.append("first_year=$firstY\nlast_year=$lastY\nfirst_epoch_day=${HijrahDate.of(firstY, 1, 1).toEpochDay()}\n")
    for (y in firstY..lastY) h.append((1..12).joinToString(",") { m -> HijrahDate.of(y, m, 1).lengthOfMonth().toString() }).append('\n')
    File(dataDir, "hijri_table.txt").writeText(h.toString())

    val g = StringBuilder("gregorian,year,month,day\n")
    var x = LocalDate.parse("2020-01-01")
    while (x.isBefore(LocalDate.parse("2040-01-01"))) {
        val hd = HijrahDate.from(x)
        g.append("$x,${hd.get(ChronoField.YEAR)},${hd.get(ChronoField.MONTH_OF_YEAR)},${hd.get(ChronoField.DAY_OF_MONTH)}\n")
        x = x.plusDays(11)
    }
    File(out, "golden_hijri.csv").writeText(g.toString())
    println("golden rows: ${sb.count { it == '\n' } - 1}, hijri years $firstY..$lastY")
}
