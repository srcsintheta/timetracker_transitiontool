use std::fs;
use std::io;
use std::io::Write;
use std::path;

use chrono::Datelike;
use chrono::TimeZone;
use directories::ProjectDirs;
use rusqlite::Connection;
use rusqlite::OpenFlags;

/*
 * Hardcoded stuff from the new db layout
 * copied from version 0.1.0
 */

const DBNAME : &str = "productivity.db";

pub const SQL_CREATE_ACT : &str =
"CREATE TABLE tt_activities (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL, 
    added TEXT NOT NULL, 
    isactive INTEGER NOT NULL DEFAULT 1, 
    hourstotal NUMERIC NOT NULL DEFAULT 0.0
    )";

pub const SQL_CREATE_HIS : &str = 
"CREATE TABLE tt_history (
    id INTEGER NOT NULL, 
    year INTEGER NOT NULL, 
    month INTEGER NOT NULL, 
    day INTEGER NOT NULL, 
    isoweek INTEGER NOT NULL, 
    isoweekyear INTEGER NOT NULL,
    hoursonday NUMERIC NOT NULL DEFAULT 0.0, 
    date TEXT NOT NULL,
    FOREIGN KEY (id) REFERENCES tt_activities(id)
    )";


fn round6 ( val : f64) -> f64
{
    (val * 1_000_000.).round() / 1_000_000.
}

struct DBOldRowActivities {
    id			: i32,
    group_id	: i32,		// disregarded for new db
    name		: String,
    added_when	: String,
    is_activated : i32,
    hours_total : f64,
}

struct DBOldRowHistory {
    id_activity  : i32,
    year		 : i32,
    month		 : i32,
    day			 : i32,
    weeknumber   : i32,
    hours_on_day : f64,
    date	     : String,
}

fn main()
{
    /*
     * Explanation Primer
     */

    println!("Small tool to transition from old timetracker to new version");
    println!("Written for self use");
    println!("Note: ");
    println!("  a) certain values hard-coded (db names etc)");
    println!("    (won't keep this tool up to date if breaking changes occur)");
    println!("  b) no graceful error checking here");
    println!("    (expect panics as soon as something doesn't work)");
    
    /*
     * Retrieve full db path of old db
     */

    let mut path   : String = Default::default();
    let mut db_old : Connection;

    println!("Enter your full db path, eg: /home/user/foo/bar/productivity.db");
    print!  ("       Your entry          : ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut path).expect("Failed to read line");
    path = path.trim().to_string();

    /*
     * Open for reading
     */

    db_old = Connection::open_with_flags(
        &path, OpenFlags::SQLITE_OPEN_READ_ONLY
        ).unwrap();

    println!("Opened {:?} read-only", path);

    /*
     * Determine path for the new db
     */

    let projdir = ProjectDirs::from("dev", "sintheta", "timetracker");
    let dcpath : path::PathBuf;
    let dbpath : path::PathBuf;

    let dcpath_exists: bool;
    let dbpath_exists: bool;

    println!();

    if let Some(d) = projdir
    {
        dcpath = d.config_dir().to_path_buf();
        dbpath = dcpath.join(DBNAME);
    }
    else 
    {
        panic!("Could not retrieve OS specific configuration folder!");
    }

    /*
     * create folder and db file if needed
     */

    dcpath_exists = dcpath.exists();
    dbpath_exists = dbpath.exists();

    if !dcpath_exists
    {
        println!("folder  doesn't exist, creating: {:?}", dcpath);
        fs::create_dir_all(&dcpath).unwrap();
    }
    if !dbpath_exists
    {
        println!("db file doesn't exist, creating: {:?}", dbpath);
    }
    /*
     * if db already exists warn user!
     */
    else
    {
        println!("db already exists at {:?}", dbpath);
        println!("NOTE: we'll update the new db w/ values from the old!");
        println!("MAKE SURE YOUR DB IS PROPERLY BACKED UP BEFORE THIS!");
        println!("(seriously just do it...");
    }

    /*
     * iterate over old db data; activities
     */

    let mut stmt = db_old
        .prepare(&format!("SELECT * FROM activities"))
        .unwrap();

    let iter = stmt.query_map([], |row| {
        Ok(DBOldRowActivities {
            id			: row.get(0)?,
            group_id	: row.get(1)?,
            name		: row.get(2)?,
            added_when	: row.get(3)?,
            is_activated: row.get(4)?,
            hours_total : row.get(5)?,

        })
    }).unwrap();

    let mut oldact : Vec<DBOldRowActivities> = Vec::new();
    for e in iter { oldact.push(e.unwrap()); }

    /*
     * iterate over old db data; history
     */

    let mut stmt = db_old
        .prepare(&format!("SELECT * FROM history"))
        .unwrap();

    let iter = stmt.query_map([], |row| {
        Ok(DBOldRowHistory {
            id_activity : row.get(0)?, 
            year		: row.get(1)?, 
            month		: row.get(2)?, 
            day			: row.get(3)?, 
            weeknumber	: row.get(4)?, 
            hours_on_day: row.get(5)?, 
            date		: row.get(6)?, 
        })
    }).unwrap();

    let mut oldhis : Vec<DBOldRowHistory> = Vec::new();
    for e in iter { oldhis.push(e.unwrap()); }

    /*
     * open new db for read/write
     */

    let db_new = Connection::open(dbpath).unwrap();

    /*
     * create tables in db (if db is new)
     */

    if !dbpath_exists
    {
        db_new.execute(SQL_CREATE_ACT, ()).unwrap();
        db_new.execute(SQL_CREATE_HIS, ()).unwrap();
    }

    /*
     * enter activities into new db
     */

    for e in oldact
    {
        db_new.execute(
                "INSERT INTO tt_activities 
                (id, name, added, isactive, hourstotal) 
                VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                	e.id,
                    e.name,
                    e.added_when,
                    e.is_activated,
                    round6(e.hours_total),
                ]).unwrap();
    }

    /*
     * enter all history into new db
     */

    for e in oldhis
    {
        let dtlocal = chrono::Local
            .with_ymd_and_hms(
                e.year, 
                e.month.try_into().unwrap(), 
                e.day.try_into().unwrap(), 
                0, 0, 0)
            .unwrap();

        db_new.execute(
            "INSERT INTO tt_history 
            (id, year, month, day, isoweek, isoweekyear, hoursonday, date) 
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                e.id_activity,
                e.year,
                e.month,
                e.day,
                e.weeknumber,
                dtlocal.iso_week().year(),
                round6(e.hours_on_day),
                e.date,
            ]).unwrap();
    }

    println!("Done, if the program ran this far it worked");
}
