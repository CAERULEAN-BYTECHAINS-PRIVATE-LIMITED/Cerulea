//! The accounts that administer one deployment.
//!
//! Several modules carry calls that change how a deployment runs. The tier
//! module takes the control plane key. The relayer module takes sponsors,
//! quotas, rate limits and throttles. Each call asks this interface whether
//! the caller administers the deployment, so every module decides the same
//! way.
//!
//! Three accounts administer a deployment. The account that installed the
//! Blueprint. An appointed Cerulea operator of the deployment. The account
//! whose public key is the control plane key of the deployment. The runtime
//! joins the three answers. A mock answers from a fixed list.

use crate::primitives::DeploymentId;

/// The interface a module asks who administers one deployment.
pub trait DeploymentStewards<AccountId> {
    /// The account that installed the Blueprint of the deployment.
    ///
    /// The answer is absent for a deployment no account installed.
    fn installer(deployment: &DeploymentId) -> Option<AccountId>;

    /// Report whether one account administers the deployment.
    fn is_steward(deployment: &DeploymentId, who: &AccountId) -> bool;
}

/// The empty interface. No account administers any deployment.
impl<AccountId> DeploymentStewards<AccountId> for () {
    fn installer(_deployment: &DeploymentId) -> Option<AccountId> {
        None
    }

    fn is_steward(_deployment: &DeploymentId, _who: &AccountId) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::DeploymentStewards;

    #[test]
    fn the_empty_interface_names_no_installer_and_no_steward() {
        let deployment = [3u8; 32];
        assert_eq!(
            <() as DeploymentStewards<u64>>::installer(&deployment),
            None
        );
        assert!(!<() as DeploymentStewards<u64>>::is_steward(
            &deployment,
            &1
        ));
        assert!(!<() as DeploymentStewards<u64>>::is_steward(&[0u8; 32], &0));
    }
}
