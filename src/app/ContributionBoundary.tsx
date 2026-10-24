import { Component, type ErrorInfo, type ReactNode } from "react";
import { Icon } from "../components/Icon";

interface ContributionBoundaryProps {
  contributionId: string;
  children: ReactNode;
}

interface ContributionBoundaryState {
  failed: boolean;
}

export class ContributionBoundary extends Component<ContributionBoundaryProps, ContributionBoundaryState> {
  state: ContributionBoundaryState = { failed: false };

  static getDerivedStateFromError(): ContributionBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error(`Frontend contribution ${this.props.contributionId} failed to render.`, error, info.componentStack);
  }

  render() {
    if (this.state.failed) {
      return (
        <div className="global-error" role="alert">
          <Icon name="alert" />
          <span>此页面内容暂时无法显示。</span>
        </div>
      );
    }
    return this.props.children;
  }
}
