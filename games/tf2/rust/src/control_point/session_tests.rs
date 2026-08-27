use crate::*;
use playsrc_collision::Hull;
use playsrc_movement::{Error as MoveError, Trace, Tracer};

#[derive(Clone)]
struct CaptureFloor;
impl Tracer for CaptureFloor {
    fn trace(&self, start: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, MoveError> {
        let fraction = if end[2] < 0.0 { (start[2] / (start[2] - end[2])).clamp(0.0, 1.0) } else { 1.0 };
        Ok(Trace { fraction, start_solid: false, all_solid: false,
            end: std::array::from_fn(|i| start[i] + (end[i] - start[i]) * fraction),
            normal: (fraction < 1.0).then_some([0.0, 0.0, 1.0]), hit: (fraction < 1.0).then_some(0), contents: u32::from(fraction < 1.0) })
    }
}
impl GameplayWorld for CaptureFloor {
    fn overlaps_model_hull(&self, model: usize, _: [f32; 3], position: [f32; 3], _: Hull) -> Result<bool, MoveError> {
        Ok((position[0] - model as f32 * 200.0).abs() < 50.0)
    }
}

#[test]
fn session_attack_defend_keeps_mini_round_entities_timer_and_scores_then_switches_teams() {
    let mut text = String::from(r#"
        {"classname" "team_control_point_master" "targetname" "master" "switch_teams" "1" "score_style" "1"}
        {"classname" "team_round_timer" "targetname" "timer" "timer_length" "270" "setup_length" "2" "start_paused" "0" "show_in_hud" "1" "OnFinished" "defenders,RoundWin,,0,-1"}
        {"classname" "game_round_win" "targetname" "defenders" "TeamNum" "2" "force_map_reset" "1" "switch_teams" "1"}
        {"classname" "logic_auto" "OnMultiNewMap" "tf_gamerules,SetBlueTeamRespawnWaveTime,2,0,-1"}
    "#);
    for stage in 0..2 {
        let a = stage * 2;
        let b = a + 1;
        text += &format!(r#"{{"classname" "team_control_point_round" "targetname" "r{stage}" "cpr_priority" "{}" "cpr_cp_names" "p{a} p{b}" "cpr_restrict_team_cap_win" "2"}}
            {{"classname" "info_player_teamspawn" "TeamNum" "3" "origin" "-200 0 1" "round_bluespawn" "r{stage}"}}
            {{"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "-400 0 1" "round_redspawn" "r{stage}"}}"#, 100 - stage);
        for point in [a, b] {
            text += &format!(r#"{{"classname" "team_control_point" "targetname" "p{point}" "point_index" "{point}" "point_default_owner" "2" "point_printname" "Point" "team_previouspoint_3_0" "p{a}"}}
                {{"classname" "trigger_capture_area" "area_cap_point" "p{point}" "model" "*{}" "area_time_to_cap" "3" "team_cancap_3" "1" "OnCapTeam2" "timer,AddTime,270,0.01,-1"}}"#, point + 1);
        }
    }
    let graph = playsrc_entity::parse(text.as_bytes(), Default::default()).unwrap();
    let bounds = (1..=4).map(|model| playsrc_entity::ModelBounds { model,
        mins: [model as f32 * 200.0 - 50.0, -50.0, 0.0], maxs: [model as f32 * 200.0 + 50.0, 50.0, 100.0] }).collect();
    let map = MapRuntime::compile(&graph, 0.015, 42, bounds).unwrap();
    let mut session = Session::new(CaptureFloor, [-200.0, 0.0, 1.0], map);
    session.select_team_choice(team_selection::TeamChoice::Blue).unwrap();
    let mut configuration = session.map.round_configuration();
    configuration.waiting_seconds = 0.0;
    session.round = round::Rules::new(configuration).unwrap();
    for _ in 0..400 { session.advance(Command::default()).unwrap(); }
    assert_eq!(session.map.control_points().unwrap().current_round(), Some(0));
    assert_eq!(session.round.respawn_waves(), [None, Some(2.0)]);
    for point in 0..4 {
        session.movement.position = [(point + 1) as f32 * 200.0, 0.0, 1.0];
        let mut captured = false;
        for _ in 0..430 {
            let snapshot = session.advance(Command::default()).unwrap();
            if snapshot.control_points.unwrap().points[point].owner == PlayerTeam::Blue { captured = true; break; }
        }
        assert!(captured, "point {point}");
        // Delayed authored AddTime is serviced after the cap, including in TeamWin.
        for _ in 0..3 { session.advance(Command::default()).unwrap(); }
        let scores = session.round.snapshot(vec![]);
        assert_eq!(scores.blue_score, point as u16 + 1);
        assert_eq!(scores.rounds_played, u32::from(point == 3));
        assert!(scores.timer.unwrap().remaining > 500.0);
        if point % 2 == 1 {
            let final_stage = point == 3;
            assert_eq!(session.round.state(), round::State::TeamWin);
            let before_timer = session.round.timer().unwrap().remaining as i32;
            for _ in 0..1020 { session.advance(Command::default()).unwrap(); }
            let round = session.round.snapshot(vec![]);
            assert_eq!(round.state, round::State::Preround);
            assert_eq!(round.timer.unwrap().state, round::TimerState::Setup);
            assert_eq!(session.team_selection.local_team(), if final_stage { PlayerTeam::Red } else { PlayerTeam::Blue });
            assert_eq!(session.map.control_points().unwrap().current_round(), Some(if final_stage { 0 } else { 1 }));
            for _ in 0..340 { session.advance(Command::default()).unwrap(); }
            let timer = session.round.timer().unwrap();
            assert!(timer.remaining > if final_stage { 260.0 } else { before_timer as f32 - 10.0 });
            if final_stage {
                assert_eq!(session.round.snapshot(vec![]).red_score, 4);
                assert!(session.map.control_points().unwrap().points().iter().all(|p| p.owner == PlayerTeam::Red));
            }
        }
    }
    session.map_input(1, b"SetTime", playsrc_entity::Variant::Integer(1)).unwrap();
    for _ in 0..100 { session.advance(Command::default()).unwrap(); }
    let defended = session.round.snapshot(vec![]);
    assert_eq!(defended.winning_team, Some(PlayerTeam::Red));
    assert_eq!(defended.win_reason, round::WIN_REASON_DEFEND_UNTIL_TIME_LIMIT);
    assert_eq!(defended.rounds_played, 2);
    session.fire_entity_input(b"master", b"SetWinner", b"3", 0.0).unwrap();
    for _ in 0..3 { session.advance(Command::default()).unwrap(); }
    assert_eq!(session.round.winning_team(), Some(PlayerTeam::Red));
    assert_eq!(session.restrictions.team_win, Some(PlayerTeam::Red), "late master I/O cannot replace the accepted timer winner");
    for _ in 0..1020 { session.advance(Command::default()).unwrap(); }
    assert_eq!(session.team_selection.local_team(), PlayerTeam::Blue);
    assert_eq!(session.round.snapshot(vec![]).blue_score, 4);
    for _ in 0..400 { session.advance(Command::default()).unwrap(); }
    session.fire_entity_input(b"p0", b"SetOwner", b"3", 0.0).unwrap();
    // A pusher acknowledgement services the same entity queue before the game
    // phase. Its capture/score outputs must remain owned by the session.
    session.apply_mover_results(&[]).unwrap();
    let first = session.advance(Command::default()).unwrap();
    assert_eq!(first.round.blue_score, 5, "first queued owner input retains its capture score");
    let last = session.map.control_points().unwrap().points()[1].identity;
    session.map_input(last, b"SetOwner", playsrc_entity::Variant::Integer(3)).unwrap();
    assert_ne!(session.round.state(), round::State::TeamWin, "enqueue is not ServiceEvents");
    let won = session.advance(Command::default()).unwrap();
    assert_eq!(won.round.blue_score, 6);
    assert_eq!(won.round.winning_team, Some(PlayerTeam::Blue));
    assert!(!won.round.full_round);
    assert!(won.round.events.iter().any(|event| matches!(event, round::Event::RoundWon { team: PlayerTeam::Blue, .. })));
}
