//! POC: natural-human-but-lean sentences under a token budget.
//!
//! Generates 100 sentences: 10 topics x nesting depth 1..=10. Each exists in
//! two forms — verbose (filler, passive, noun phrases, throat-clearing) and
//! lean (active voice, strong verbs, grammar kept, fat cut). Asserts the lean
//! form is shorter in tokens at every depth, and prints a savings table when
//! run via `cargo test --test nested_sentences -- --nocapture`.

const MIN_DEPTH: usize = 1;
const MAX_DEPTH: usize = 10;

struct Topic {
    name: &'static str,
    verbose_base: &'static str,
    lean_base: &'static str,
}

const TOPICS: &[Topic] = &[
    Topic {
        name: "retry",
        verbose_base: "In order to fix the problem, the worker has to make a decision to retry the failed jobs",
        lean_base: "The worker retries the failed jobs",
    },
    Topic {
        name: "cache",
        verbose_base: "It should be noted that the cache is actually responsible for making the reads fast",
        lean_base: "The cache speeds up reads",
    },
    Topic {
        name: "pool",
        verbose_base: "Due to the fact that the pool is basically exhausted, it is necessary to add connections",
        lean_base: "The pool is exhausted, so we add connections",
    },
    Topic {
        name: "timeout",
        verbose_base: "The reason why the request fails is because the timeout is being hit on a regular basis",
        lean_base: "The request fails because it hits the timeout",
    },
    Topic {
        name: "queue",
        verbose_base: "In the event that the queue is full, what happens is that jobs are being dropped",
        lean_base: "A full queue drops jobs",
    },
    Topic {
        name: "index",
        verbose_base: "What needs to be done is to make an update to the index in order for queries to be fast",
        lean_base: "We update the index so queries stay fast",
    },
    Topic {
        name: "auth",
        verbose_base: "It is important to note that the token check is the thing that is actually rejecting users",
        lean_base: "The token check rejects users",
    },
    Topic {
        name: "deploy",
        verbose_base: "As a result of the deploy being broken, it is required to perform a rollback of the change",
        lean_base: "The deploy broke, so we roll back the change",
    },
    Topic {
        name: "log",
        verbose_base: "For the purpose of debugging, it is a good idea to actually enable verbose logging",
        lean_base: "We enable verbose logging for debugging",
    },
    Topic {
        name: "scale",
        verbose_base: "Due to the fact that load is basically growing, there is a need to make a scale-up of the nodes",
        lean_base: "Load grows, so we scale up the nodes",
    },
];

const VERBOSE_NEST: &[&str] = &[
    ", which is due to the fact that the service is basically overloaded",
    ", and this is the reason why the latency is actually increasing over time",
    ", which in turn means that the timeout is being hit on the downstream calls",
    ", and as a result it becomes necessary for the system to make a retry",
    ", which is something that happens because the cache is being cold at startup",
    ", and the reason for that is that the upstream is actually down at the moment",
    ", which leads to a situation where the jobs are being dropped from the queue",
    ", and that is caused by the fact that the pool is basically exhausted of workers",
    ", which in the end means that the throughput is being degraded for the users",
];

const LEAN_NEST: &[&str] = &[
    " because the service is overloaded",
    ", raising latency",
    ", hitting the downstream timeout",
    ", forcing a retry",
    " because the cache is cold at startup",
    " because the upstream is down",
    ", dropping queued jobs",
    " because the pool has no free workers",
    ", degrading throughput",
];

fn verbose_nested(topic: &Topic, depth: usize) -> String {
    let mut s = topic.verbose_base.to_string();
    for clause in VERBOSE_NEST.iter().take(depth.saturating_sub(1)) {
        s.push_str(clause);
    }
    s.push('.');
    s
}

fn lean_nested(topic: &Topic, depth: usize) -> String {
    let mut s = topic.lean_base.to_string();
    for clause in LEAN_NEST.iter().take(depth.saturating_sub(1)) {
        s.push_str(clause);
    }
    s.push('.');
    s
}

fn tokens(s: &str) -> usize {
    s.split_whitespace().count()
}

#[test]
fn lean_is_never_longer_at_any_depth() {
    for topic in TOPICS {
        for depth in MIN_DEPTH..=MAX_DEPTH {
            let v = tokens(&verbose_nested(topic, depth));
            let l = tokens(&lean_nested(topic, depth));
            assert!(
                l < v,
                "topic {} depth {}: lean ({}) must be < verbose ({})\n  V: {}\n  L: {}",
                topic.name,
                depth,
                l,
                v,
                verbose_nested(topic, depth),
                lean_nested(topic, depth)
            );
        }
    }
}

#[test]
fn savings_do_not_collapse_as_nesting_grows() {
    // Core claim: the lean rules keep their leverage as nesting deepens.
    // (a) every individual sentence clears a token-saving floor, and
    // (b) the *average* saving at the deepest level is at least as good as
    //     at the shallowest level — nesting does not erode the win.
    const FLOOR_PCT: f64 = 40.0;

    for topic in TOPICS {
        for depth in MIN_DEPTH..=MAX_DEPTH {
            let v = tokens(&verbose_nested(topic, depth)) as f64;
            let l = tokens(&lean_nested(topic, depth)) as f64;
            let saving = (v - l) / v * 100.0;
            assert!(
                saving >= FLOOR_PCT,
                "topic {} depth {}: saving {:.1}% < {FLOOR_PCT}% floor",
                topic.name,
                depth,
                saving
            );
        }
    }

    let avg = |depth: usize| {
        let mut v = 0_usize;
        let mut l = 0_usize;
        for topic in TOPICS {
            v += tokens(&verbose_nested(topic, depth));
            l += tokens(&lean_nested(topic, depth));
        }
        (v - l) as f64 / v as f64 * 100.0
    };
    let shallow = avg(MIN_DEPTH);
    let deep = avg(MAX_DEPTH);
    assert!(
        deep >= shallow,
        "deep nesting eroded savings: depth {MIN_DEPTH}={shallow:.1}% vs depth {MAX_DEPTH}={deep:.1}%"
    );
}

#[test]
fn exactly_one_hundred_sentences_generated() {
    let mut count = 0;
    for topic in TOPICS {
        for depth in MIN_DEPTH..=MAX_DEPTH {
            let _ = verbose_nested(topic, depth);
            let _ = lean_nested(topic, depth);
            count += 1;
        }
    }
    assert_eq!(count, 100);
    assert_eq!(TOPICS.len() * MAX_DEPTH, 100);
}

#[test]
fn print_savings_table() {
    let mut totals = (0_u64, 0_u64);
    println!("\n depth | avg verbose tok | avg lean tok | avg saving %");
    println!("-------+----------------+--------------+-------------");
    for depth in MIN_DEPTH..=MAX_DEPTH {
        let mut v_sum = 0_usize;
        let mut l_sum = 0_usize;
        for topic in TOPICS {
            v_sum += tokens(&verbose_nested(topic, depth));
            l_sum += tokens(&lean_nested(topic, depth));
        }
        let n = TOPICS.len();
        let avg_v = v_sum / n;
        let avg_l = l_sum / n;
        let saving = (avg_v - avg_l) as f64 / avg_v as f64 * 100.0;
        totals.0 += v_sum as u64;
        totals.1 += l_sum as u64;
        println!(
            " {:>5} | {:>14} | {:>12} | {:>9.1}%",
            depth, avg_v, avg_l, saving
        );
    }
    let total_saving = (totals.0 - totals.1) as f64 / totals.0 as f64 * 100.0;
    println!("-------+----------------+--------------+-------------");
    println!(
        " total | {:>14} | {:>12} | {:>9.1}%  ({} sentences)",
        totals.0,
        totals.1,
        total_saving,
        TOPICS.len() * MAX_DEPTH
    );
    assert!(
        total_saving > 40.0,
        "overall saving {total_saving:.1}% too low"
    );
}
