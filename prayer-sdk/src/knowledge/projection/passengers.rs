use super::super::*;

impl RuntimeService {
    /// Return the passenger boards currently represented in SDK knowledge.
    pub fn arbitrage_passenger_boards(
        &self,
        state: &prayer_runtime::economy::EconomyReadState,
        _include_origin_jump: bool,
    ) -> Vec<prayer_state::PassengerState> {
        let knowledge = self.knowledge_state.read();
        if knowledge.station_passengers.is_empty() {
            return vec![state.passengers.clone()];
        }
        let mut boards = knowledge
            .station_passengers
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let state_station = state.passengers.station.trim();
        if !state_station.is_empty() && !knowledge.station_passengers.contains_key(state_station) {
            boards.push(state.passengers.clone());
        }
        boards
    }
}
