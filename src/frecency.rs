use rusqlite::{Connection, Error as SqliteError, Result as SqliteResult};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FrecencyError {
    #[error("{0}")]
    SqliteError(#[from] SqliteError),

    #[error("invalid max visit num")]
    InvalidMaxVisitNum,
}

pub type Result<T> = std::result::Result<T, FrecencyError>;

const LAMBDA: f64 = std::f64::consts::LN_2 / 30.0;

const DAY_IN_MILLI_SEC: f64 = 86_400_000.0;

pub struct DB {
    conn: Connection,
    max_visit_log_num: usize,
}

impl DB {
    pub fn new<P>(dbpath: P, max_visit_log_num: Option<usize>) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        if let Some(max_visit_log_num) = max_visit_log_num.as_ref() {
            if *max_visit_log_num == 0 {
                return Err(FrecencyError::InvalidMaxVisitNum);
            }
        }

        let conn = Connection::open(dbpath.as_ref())?;
        Ok(Self {
            conn,
            max_visit_log_num: max_visit_log_num.unwrap_or(20),
        })
    }
}

pub fn create_tables(db: &DB) -> Result<()> {
    db.conn.execute_batch(
        r"
        CREATE TABLE IF NOT EXISTS scores(
            path, TEXT PRIMARY KEY,
            score RATE NOT NULL,
            last_visit_in_milli_sec INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS visits(
            path, TEXT,
            visit_in_milli_sec INTEGER NOT NULL
        );


        CREATE INDEX IF NOT EXISTS last_visit_in_scores ON scores(last_visit_in_milli_sec);
        CREATE INDEX IF NOT EXISTS score_in_scores ON scores(score);
        CREATE INDEX IF NOT EXISTS path_in_visits ON visits(path);
        CREATE INDEX IF NOT EXISTS path_and_visit_in_visits ON visits(path, visit_in_milli_sec);

        ",
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn drop_tables(db: &DB) -> Result<()> {
    db.conn.execute_batch(
        r"

        DROP INDEX IF EXISTS last_visit_in_scores;
        DROP INDEX IF EXISTS score_in_scores;
        DROP INDEX IF EXISTS path_in_visits;
        DROP INDEX IF EXISTS path_and_visit_in_visits;

        DROP TABLE IF EXISTS scores;
        DROP TABLE IF EXISTS visits;

        ",
    )?;
    Ok(())
}

pub fn remove_paths(db: &mut DB, paths: &[&str]) -> Result<()> {
    let tx = db.conn.transaction()?;
    for each_path in paths {
        tx.execute(
            r" DELETE FROM visits where path = :path",
            &[(":path", each_path)],
        )?;

        tx.execute(
            r" DELETE FROM scores where path = :path",
            &[(":path", each_path)],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn fetch_scores(db: &DB, limit: Option<usize>) -> Result<Vec<(String, f64)>> {
    let fetch_query = match limit {
        Some(limit) => format!(
            " SELECT score, path
            FROM scores
            ORDER BY score DESC
            LIMIT {limit} ",
        ),
        None => "SELECT score, path
            FROM scores
            ORDER BY score DESC"
            .to_string(),
    };

    let mut stmt = db.conn.prepare_cached(&fetch_query)?;

    let records = stmt.query_and_then([], |row| -> SqliteResult<(String, f64)> {
        let score: f64 = row.get(0)?;
        let path: String = row.get(1)?;
        Ok((path, score))
    })?;
    let mut scores = Vec::new();
    for each in records {
        let (path, score) = each?;
        scores.push((path, score));
    }
    Ok(scores)
}

pub fn fetch_last_visit(db: &DB, limit: Option<usize>) -> Result<Vec<(String, f64)>> {
    let fetch_query = match limit {
        Some(limit) => format!(
            " SELECT score, path
            FROM scores
            ORDER BY last_visit_in_milli_sec DESC
            LIMIT {limit} ",
        ),
        None => "SELECT score, path
            FROM scores
            ORDER BY last_visit_in_milli_sec  DESC"
            .to_string(),
    };

    let mut stmt = db.conn.prepare_cached(&fetch_query)?;

    let records = stmt.query_and_then([], |row| -> SqliteResult<(String, f64)> {
        let score: f64 = row.get(0)?;
        let path: String = row.get(1)?;
        Ok((path, score))
    })?;
    let mut scores = Vec::new();
    for each in records {
        let (path, score) = each?;
        scores.push((path, score));
    }
    Ok(scores)
}

pub fn fetch_visits(db: &DB, path: &str) -> Result<Vec<u64>> {
    let fetch_query = " SELECT visit_in_milli_sec
            FROM visits
            WHERE path = :path
            ORDER BY visit_in_milli_sec ASC ";

    let mut stmt = db.conn.prepare_cached(fetch_query)?;
    let mut visits = Vec::new();

    let records = stmt.query_map(&[(":path", &path)], |row| row.get(0))?;
    for each in records {
        let visit: u64 = each?;
        visits.push(visit);
    }
    Ok(visits)
}

pub fn calc_score(latest_visit: u64, past_visits_milli_sec: &[u64]) -> f64 {
    debug_assert!(!past_visits_milli_sec.is_empty());
    past_visits_milli_sec
        .iter()
        .map(|each_past_visits| {
            let age =
                (latest_visit as i64 - *each_past_visits as i64).max(0) as f64 / DAY_IN_MILLI_SEC;
            (-LAMBDA * age).exp()
        })
        .sum::<f64>()
}

fn store_score_with_latest_visit(
    db: &mut DB,
    path: &str,
    is_first_visits: bool,
    score: f64,
    latest_visit: u64,
    remove_visits: &[u64],
) -> Result<()> {
    let tx = db.conn.transaction()?;

    if is_first_visits {
        tx.execute(
            &format!(
                "INSERT INTO scores(path, score, last_visit_in_milli_sec)VALUES(:path, {score}, {latest_visit})"
            ),
            &[(":path", &path)],
        )?;

        tx.execute(
            &format!("INSERT INTO visits(path, visit_in_milli_sec )VALUES(:path, {latest_visit})"),
            &[(":path", &path)],
        )?;
    } else {
        tx.execute(
            &format!("UPDATE  scores set score = {score}, last_visit_in_milli_sec = {latest_visit} WHERE path =:path "),
            &[(":path", &path)],
        )?;

        tx.execute(
            &format!("INSERT INTO visits(path, visit_in_milli_sec )VALUES(:path, {latest_visit})"),
            &[(":path", &path)],
        )?;

        if !remove_visits.is_empty() {
            let visits_in = remove_visits
                .iter()
                .map(|v| format!("{v}"))
                .collect::<Vec<String>>()
                .join(",");

            tx.execute(
                &format!(
                    "DELETE FROM visits WHERE path = :path and visit_in_milli_sec in ({visits_in})"
                ),
                &[(":path", &path)],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn add_visit(db: &mut DB, path: &str, latest_visit_in_milli_sec: u64) -> Result<()> {
    let mut past_visits = fetch_visits(db, path)?;
    let is_first_visits = past_visits.is_empty();
    past_visits.push(latest_visit_in_milli_sec);

    let new_score = calc_score(latest_visit_in_milli_sec, past_visits.as_ref());
    let remove_visits = if past_visits.len() > db.max_visit_log_num {
        let sreshold = past_visits.len() - db.max_visit_log_num;
        let (remove_visits, _) = past_visits.split_at(sreshold);
        remove_visits.to_vec()
    } else {
        vec![]
    };
    store_score_with_latest_visit(
        db,
        path,
        is_first_visits,
        new_score,
        latest_visit_in_milli_sec,
        remove_visits.as_ref(),
    )?;
    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct TestDB {
        path: PathBuf,
    }

    impl Drop for TestDB {
        fn drop(&mut self) {
            fs::remove_file(&self.path).unwrap();
        }
    }

    fn new_test_db() -> TestDB {
        let mut p = PathBuf::new();
        p.push("/tmp/frecency_test");
        if !p.as_path().exists() {
            fs::create_dir_all(p.as_path()).unwrap();
        }
        let uid = Uuid::new_v4();
        p.push(format!("{uid}.db3"));
        TestDB { path: p }
    }

    #[test]
    fn test_queries() {
        let test_db = new_test_db();
        let mut db = DB::new(&test_db.path, Some(3)).unwrap();
        drop_tables(&db).unwrap();

        create_tables(&db).unwrap();

        {
            let result = add_visit(&mut db, "p1", 1557159200000);
            assert!(result.is_ok());
            let visits = fetch_visits(&db, "p1").unwrap();
            assert_eq!(visits, vec![1557159200000]);
            let scores = fetch_scores(&db, None).unwrap();
            assert_eq!(scores, vec![("p1".to_string(), 1f64)]);
        }

        {
            let result = add_visit(&mut db, "p1", 1558159200000);
            println!("{:?}", result);
            assert!(result.is_ok());
            let visits = fetch_visits(&db, "p1").unwrap();
            assert_eq!(visits, vec![1557159200000, 1558159200000]);
            let scores = fetch_scores(&db, Some(1000)).unwrap();
            assert_eq!(scores, vec![("p1".to_string(), 1.76535316833351f64)]);
        }

        {
            let result = add_visit(&mut db, "p1", 1558180800000);
            println!("{:?}", result);
            assert!(result.is_ok());
            let visits = fetch_visits(&db, "p1").unwrap();
            assert_eq!(visits, vec![1557159200000, 1558159200000, 1558180800000]);
            let scores = fetch_scores(&db, None).unwrap();
            assert_eq!(scores, vec![("p1".to_string(), 2.755185482271559)]);
        }

        {
            let result = add_visit(&mut db, "p2", 1558180800000);
            println!("{:?}", result);
            assert!(result.is_ok());
            let visits = fetch_visits(&db, "p1").unwrap();
            assert_eq!(visits, vec![1557159200000, 1558159200000, 1558180800000]);

            let visits = fetch_visits(&db, "p2").unwrap();
            assert_eq!(visits, vec![1558180800000]);

            let scores = fetch_scores(&db, Some(1000)).unwrap();
            assert_eq!(
                scores,
                vec![
                    ("p1".to_string(), 2.755185482271559),
                    ("p2".to_string(), 1f64)
                ]
            );
        }

        {
            let result = add_visit(&mut db, "p1", 1559180800000);
            println!("{:?}", result);
            assert!(result.is_ok());
            let visits = fetch_visits(&db, "p1").unwrap();
            assert_eq!(visits, vec![1558159200000, 1558180800000, 1559180800000]);

            let visits = fetch_visits(&db, "p2").unwrap();
            assert_eq!(visits, vec![1558180800000]);

            let scores = fetch_scores(&db, None).unwrap();
            assert_eq!(
                scores,
                vec![
                    ("p1".to_string(), 3.1086899382030277),
                    ("p2".to_string(), 1f64)
                ]
            );
        }
    }

    #[test]
    fn test_cals_score() {
        {
            let score = calc_score(1558159200000, &[1558159200000]);

            assert_eq!(score, 1f64);
        }

        {
            let score = calc_score(1558180800000, &[1558159200000, 1558180800000]);
            assert_eq!(score, 1.9942404238175473);
        }

        {
            let score = calc_score(
                1558180800000,
                &[1557159200000, 1558159200000, 1558180800000],
            );
            assert_eq!(score, 2.755185482271559);
        }
    }
}
